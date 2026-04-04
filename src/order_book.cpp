#include "order_book.hpp"
#include <iostream>

bool OrderBook::addOrder(Order* order) {
    std::lock_guard<std::mutex> lock(book_mutex);
    if (!order || order->qty <= 0) return false;
    
    // Check if order ID already exists
    if (order_map.find(order->id) != order_map.end()) return false;
    
    Price price = order->price;
    
    if (order->side == Side::BUY) {
        // If price level doesn't exist, create it (std::map does this automatically)
        // operator[] creates a default PriceLevel if key doesn't exist
        if (bids.find(price) == bids.end()) {
            bids[price] = PriceLevel(price);
        }
        bids[price].add(order);
        
        // Update best bid cache
        if (price > best_bid_price) {
            best_bid_price = price;
        }
    } else { // SELL
        if (asks.find(price) == asks.end()) {
            asks[price] = PriceLevel(price);
        }
        asks[price].add(order);
        
        // Update best ask cache
        if (price < best_ask_price) {
            best_ask_price = price;
        }
    }
    
    // Add to order map for O(1) lookup
    order_map[order->id] = order;
    return true;
}

bool OrderBook::cancelOrder(OrderID id) {
    std::lock_guard<std::mutex> lock(book_mutex);
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
        best_ask_price = INT64_MAX;
    } else {
        // std::map with std::less keeps lowest price at begin()
        best_ask_price = asks.begin()->first;
    }
}

void OrderBook::match() {
    while (canMatch()) {
        // Get best bid and ask levels
        PriceLevel& bid_level = bids.begin()->second;
        PriceLevel& ask_level = asks.begin()->second;
        
        Order* buy_order = bid_level.head;
        Order* sell_order = ask_level.head;
        
        // Determine fill quantity
        Quantity fill_qty = std::min(buy_order->qty, sell_order->qty);
        
        // Execute fill (price = resting order's price, typically the ask)
        // TODO: Generate trade execution report here
        
        buy_order->qty -= fill_qty;
        sell_order->qty -= fill_qty;
        
        // Remove fully filled orders
        if (buy_order->qty == 0) {
            bid_level.remove(buy_order);
            order_map.erase(buy_order->id);
            // TODO: Return order to object pool instead of delete
        }
        
        if (sell_order->qty == 0) {
            ask_level.remove(sell_order);
            order_map.erase(sell_order->id);
        }
        
        // Clean up empty price levels
        if (bid_level.isEmpty()) {
            bids.erase(bids.begin());
            updateBestBid();
        }
        
        if (ask_level.isEmpty()) {
            asks.erase(asks.begin());
            updateBestAsk();
        }
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

void OrderBook::printBook() const {
    std::cout << "=== ORDER BOOK ===" << std::endl;
    
    std::cout << "ASKS (sell orders):" << std::endl;
    for (auto it = asks.rbegin(); it != asks.rend(); ++it) {
        std::cout << "  $" << it->first / 100.0 << " : " 
                  << it->second.total_volume << " shares" << std::endl;
    }
    
    std::cout << "-------------------" << std::endl;
    
    std::cout << "BIDS (buy orders):" << std::endl;
    for (const auto& [price, level] : bids) {
        std::cout << "  $" << price / 100.0 << " : " 
                  << level.total_volume << " shares" << std::endl;
    }
}
