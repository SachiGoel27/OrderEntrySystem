#pragma once
#include "types.hpp"
#include <array>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <mutex>
#include <vector>

enum class Side { BUY, SELL };
enum class OrderType { LIMIT, MARKET };
enum class TimeInForce { GTC, IOC, FOK };

struct Order {
    OrderID id;
    Side side;
    OrderType type = OrderType::LIMIT;
    TimeInForce tif = TimeInForce::GTC;
    Price price;
    Quantity qty;

    // Doubly-linked list on the book; when idle in OrderPool, `next` is the Treiber freelist link.
    Order* next = nullptr;
    Order* prev = nullptr;
};

/// Tagged head for the Treiber stack: ABA-safe via monotonic generation counter.
struct alignas(16) FreeHead {
    Order* ptr = nullptr;
    std::uint64_t gen = 0;
};

static_assert(
    sizeof(FreeHead) == 16,
    "FreeHead must be 16 bytes for typical lock-free 128-bit wide atomics");

/// Lock-free Treiber stack over a fixed slab. Overflow uses `new Order()` (tracked for destructor).
///
/// Shutdown / lifetime (important):
/// - `~OrderPool()` drains the freelist then `delete`s every pointer recorded in `overflow_allocations_`.
///   Call only when no thread will `acquire`/`release` and no `Order*` from this pool is still in use
///   on the book (typical: engine stopped, book drained, all orders released back to the pool).
/// - The static singleton is destroyed at process exit; other threads must already be joined or idle.
class OrderPool {
private:
    static constexpr std::size_t kPoolSize = 10000;
    alignas(64) std::array<Order, kPoolSize> storage_{};
    std::atomic<FreeHead> free_head_{FreeHead{nullptr, 0}};

    std::mutex overflow_mutex_;
    std::vector<Order*> overflow_allocations_;

    bool is_slab_pointer(const Order* o) const noexcept {
        const Order* begin = storage_.data();
        const Order* end = begin + kPoolSize;
        return o >= begin && o < end;
    }

    void init_free_list() {
        Order* h = nullptr;
        for (std::size_t i = kPoolSize; i-- > 0;) {
            Order& o = storage_[i];
            o.next = h;
            o.prev = nullptr;
            h = &o;
        }
        free_head_.store(FreeHead{h, 0}, std::memory_order_release);
    }

    /// Best-effort: pop entire freelist so destructor can release overflow nodes safely.
    void drain_freelist_for_shutdown() noexcept {
        for (;;) {
            FreeHead cur = free_head_.load(std::memory_order_acquire);
            if (!cur.ptr) {
                return;
            }
            Order* top = cur.ptr;
            Order* nxt = top->next;
            FreeHead desired{nxt, cur.gen + 1};
            (void)free_head_.compare_exchange_weak(
                cur,
                desired,
                std::memory_order_acq_rel,
                std::memory_order_acquire);
        }
    }

public:
    OrderPool() { init_free_list(); }
    OrderPool(const OrderPool&) = delete;
    OrderPool& operator=(const OrderPool&) = delete;

    ~OrderPool() {
        drain_freelist_for_shutdown();
        std::lock_guard<std::mutex> lock(overflow_mutex_);
        for (Order* o : overflow_allocations_) {
            if (o && !is_slab_pointer(o)) {
                delete o;
            }
        }
        overflow_allocations_.clear();
    }

    /// Hot path: lock-free pop with generation tag; allocates on exhaustion (tracked).
    Order* acquire() {
        for (;;) {
            FreeHead cur = free_head_.load(std::memory_order_acquire);
            if (!cur.ptr) {
                std::unique_ptr<Order> o(new Order());
                std::lock_guard<std::mutex> lock(overflow_mutex_);
                overflow_allocations_.push_back(o.get());
                return o.release();
            }
            Order* h = cur.ptr;
            Order* nxt = h->next;
            FreeHead desired{nxt, cur.gen + 1};
            if (free_head_.compare_exchange_weak(
                    cur,
                    desired,
                    std::memory_order_acq_rel,
                    std::memory_order_acquire)) {
                return h;
            }
        }
    }

    /// Hot path: lock-free push with generation tag.
    void release(Order* o) {
        if (!o) {
            return;
        }
        o->prev = nullptr;
        for (;;) {
            FreeHead cur = free_head_.load(std::memory_order_acquire);
            o->next = cur.ptr;
            FreeHead desired{o, cur.gen + 1};
            if (free_head_.compare_exchange_weak(
                    cur,
                    desired,
                    std::memory_order_acq_rel,
                    std::memory_order_acquire)) {
                return;
            }
        }
    }

    static OrderPool& getInstance() {
        static OrderPool instance;
        return instance;
    }
};

// Note: libstdc++ may set std::atomic<FreeHead>::is_always_lock_free to false on some targets
// (e.g. MinGW) even when the implementation still uses a single wide CAS where available.
// The tagged Treiber stack remains correct either way; only contention cost may differ.
