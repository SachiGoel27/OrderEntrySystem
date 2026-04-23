#include "order_book.hpp"
#include "types.hpp"
#include <iostream>
#include <functional>
#include <sstream>
#include <string>
#include <vector>

namespace {

struct LevelSnap {
    Price price;
    Quantity qty;
};

struct BookSnapCopy {
    Price best_bid{};
    Price best_ask{};
    bool has_ask{false};
    std::uint32_t connections{};
    std::vector<LevelSnap> bids;
    std::vector<LevelSnap> asks;
};

std::string build_book_json_from_copy(const BookSnapCopy& c) {
    std::ostringstream oss;
    oss << "{\"type\":\"book\",\"connections\":" << c.connections
        << ",\"best_bid_cached\":" << c.best_bid << ",\"best_ask_cached\":";
    if (c.has_ask) {
        oss << c.best_ask;
    } else {
        oss << "null";
    }
    oss << ",\"best_bid\":" << c.best_bid << ",\"best_ask\":";
    if (c.has_ask) {
        oss << c.best_ask;
    } else {
        oss << "null";
    }

    oss << ",\"bids\":[";
    for (std::size_t i = 0; i < c.bids.size(); ++i) {
        if (i) {
            oss << ',';
        }
        oss << "{\"price\":" << c.bids[i].price << ",\"qty\":" << c.bids[i].qty << "}";
    }
    oss << "],\"asks\":[";
    for (std::size_t i = 0; i < c.asks.size(); ++i) {
        if (i) {
            oss << ',';
        }
        oss << "{\"price\":" << c.asks[i].price << ",\"qty\":" << c.asks[i].qty << "}";
    }
    oss << "]}";
    return oss.str();
}

} // namespace

bool OrderBook::addOrder(Order* order, std::function<void(OrderID, Price, Quantity)> onTradeExecution) {
    std::lock_guard<std::recursive_mutex> lock(book_mutex);
    if (!order || order->qty <= 0) return false;
    
    // Check if order ID already exists
    if (order_map.find(order->id) != order_map.end()) return false;
    
    // 1. Fill-Or-Kill (FOK) Check
    if (order->tif == TimeInForce::FOK) {
        Quantity available_qty = 0;
        if (order->side == Side::BUY) {
            for (auto it = asks.begin(); it != asks.end(); ++it) {
                if (order->type == OrderType::LIMIT && it->first > order->price) break;
                available_qty += it->second.total_volume;
                if (available_qty >= order->qty) break;
            }
        } else {
            for (auto it = bids.begin(); it != bids.end(); ++it) {
                if (order->type == OrderType::LIMIT && it->first < order->price) break;
                available_qty += it->second.total_volume;
                if (available_qty >= order->qty) break;
            }
        }
        
        // If not enough liquidity, kill the order immediately
        if (available_qty < order->qty) {
            // Cancelled
            return false;
        }
    }

    // 2. Immediate Matching for Market/IOC/FOK & Crossing Limits
    while (order->qty > 0) {
        Price match_price = 0;
        Order* resting_order = nullptr;
        
        if (order->side == Side::BUY) {
            if (asks.empty()) break;
            auto best_ask = asks.begin();
            if (order->type == OrderType::LIMIT && best_ask->first > order->price) break; // Limit not met
            
            resting_order = best_ask->second.head;
            match_price = best_ask->first;
        } else {
            if (bids.empty()) break;
            auto best_bid = bids.begin();
            if (order->type == OrderType::LIMIT && best_bid->first < order->price) break; // Limit not met
            
            resting_order = best_bid->second.head;
            match_price = best_bid->first;
        }
        
        // Execute Trade
        Quantity fill_qty = std::min(order->qty, resting_order->qty);
        
        if (onTradeExecution) {
            onTradeExecution(order->id, match_price, fill_qty);
            onTradeExecution(resting_order->id, match_price, fill_qty);
        }
        
        order->qty -= fill_qty;
        resting_order->qty -= fill_qty;
        
        // Remove filled resting order
        if (resting_order->qty == 0) {
            if (order->side == Side::BUY) {
                asks.begin()->second.remove(resting_order);
                order_map.erase(resting_order->id);
                OrderPool::getInstance().release(resting_order);
                if (asks.begin()->second.isEmpty()) {
                    asks.erase(asks.begin());
                    updateBestAsk();
                }
            } else {
                bids.begin()->second.remove(resting_order);
                order_map.erase(resting_order->id);
                OrderPool::getInstance().release(resting_order);
                if (bids.begin()->second.isEmpty()) {
                    bids.erase(bids.begin());
                    updateBestBid();
                }
            }
        }
    }
    
    // 3. Handle Remaining Quantity
    if (order->qty > 0) {
        if (order->type == OrderType::MARKET || order->tif == TimeInForce::IOC) {
            // Market orders and IOCs do not rest on the book. Unfilled portion is cancelled.
            return true; // Partially or entirely "failed to fill" but successfully processed
        }
    
        // Add remaining limit order to the book
        Price price = order->price;
        
        if (order->side == Side::BUY) {
            if (bids.find(price) == bids.end()) bids[price] = PriceLevel(price);
            bids[price].add(order);
            if (price > best_bid_price) best_bid_price = price;
        } else { // SELL
            if (asks.find(price) == asks.end()) asks[price] = PriceLevel(price);
            asks[price].add(order);
            if (price < best_ask_price) best_ask_price = price;
        }
        
        order_map[order->id] = order;
    }
    
    return true;
}

bool OrderBook::cancelOrder(OrderID id) {
    std::lock_guard<std::recursive_mutex> lock(book_mutex);
    // O(1) lookup in hash map
    auto it = order_map.find(id);
    if (it == order_map.end()) return false;
    
    Order* order = it->second;
    Price price = order->price;
    
    if (order->side == Side::BUY) {
        auto level_it = bids.find(price);
        if (level_it != bids.end()) {
            level_it->second.remove(order);
            
            // Remove price level if empty
            if (level_it->second.isEmpty()) {
                bids.erase(level_it);
                if (price == best_bid_price) {
                    updateBestBid();
                }
            }
        }
    } else { // SELL
        auto level_it = asks.find(price);
        if (level_it != asks.end()) {
            level_it->second.remove(order);
            
            if (level_it->second.isEmpty()) {
                asks.erase(level_it);
                if (price == best_ask_price) {
                    updateBestAsk();
                }
            }
        }
    }
    
    order_map.erase(it);
    return true;
}

void OrderBook::updateBestBid() {
    if (bids.empty()) {
        best_bid_price = 0;
    } else {
        // std::map with std::greater keeps highest price at begin()
        best_bid_price = bids.begin()->first;
    }
}

void OrderBook::updateBestAsk() {
    if (asks.empty()) {
        best_ask_price = NO_ASK;
    } else {
        // std::map with std::less keeps lowest price at begin()
        best_ask_price = asks.begin()->first;
    }
}

bool OrderBook::canMatch() const {
    return !bids.empty() && !asks.empty() && best_bid_price >= best_ask_price;
}

Price OrderBook::getBestBid() const {
    return best_bid_price;
}

Price OrderBook::getBestAsk() const {
    return best_ask_price;
}

Quantity OrderBook::getVolumeAtPrice(Side side, Price price) const {
    if (side == Side::BUY) {
        auto it = bids.find(price);
        if (it != bids.end()) {
            return it->second.total_volume;
        }
    } else {
        auto it = asks.find(price);
        if (it != asks.end()) {
            return it->second.total_volume;
        }
    }
    return 0;
}

Order* OrderBook::getOrder(OrderID id) const {
    auto it = order_map.find(id);
    if (it != order_map.end()) {
        return it->second;
    }
    return nullptr;
}

std::string OrderBook::snapshot_top_json(std::size_t max_levels, std::uint32_t connections) const {
    BookSnapCopy c;
    c.connections = connections;
    {
        std::lock_guard<std::recursive_mutex> lock(book_mutex);
        c.best_bid = best_bid_price;
        c.has_ask = (best_ask_price != NO_ASK);
        c.best_ask = best_ask_price;
        for (auto it = bids.begin(); it != bids.end() && c.bids.size() < max_levels; ++it) {
            c.bids.push_back(LevelSnap{it->first, it->second.total_volume});
        }
        for (auto it = asks.begin(); it != asks.end() && c.asks.size() < max_levels; ++it) {
            c.asks.push_back(LevelSnap{it->first, it->second.total_volume});
        }
    }
    return build_book_json_from_copy(c);
}

void OrderBook::printBook() const {
    std::cout << "=== ORDER BOOK ===" << std::endl;
    
    std::cout << "ASKS (sell orders):" << std::endl;
    for (auto it = asks.rbegin(); it != asks.rend(); ++it) {
        std::cout << "  $" << priceDisplay(it->first) << " : " 
                  << it->second.total_volume << " shares" << std::endl;
    }
    
    std::cout << "-------------------" << std::endl;
    
    std::cout << "BIDS (buy orders):" << std::endl;
    for (const auto& [price, level] : bids) {
        std::cout << "  $" << priceDisplay(price) << " : " 
                  << level.total_volume << " shares" << std::endl;
    }
}
