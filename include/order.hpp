#pragma once
#include "types.hpp"
#include <vector>
#include <mutex>
#include <memory>

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

    // pointers for doubly linked list (can change this to array backed queue later if needed)
    Order* next = nullptr;
    Order* prev = nullptr;
};

class OrderPool {
private:
    std::vector<Order*> pool;
    std::mutex pool_mutex;

public:
    OrderPool() {
        // Pre-allocate some orders to speed up initial allocation
        for (int i = 0; i < 10000; ++i) {
            pool.push_back(new Order());
        }
    }

    ~OrderPool() {
        for (Order* o : pool) {
            delete o;
        }
    }

    Order* acquire() {
        std::lock_guard<std::mutex> lock(pool_mutex);
        if (pool.empty()) {
            return new Order();
        }
        Order* o = pool.back();
        pool.pop_back();
        return o;
    }

    void release(Order* o) {
        o->next = nullptr;
        o->prev = nullptr;
        std::lock_guard<std::mutex> lock(pool_mutex);
        pool.push_back(o);
    }
    
    static OrderPool& getInstance() {
        static OrderPool instance;
        return instance;
    }
};