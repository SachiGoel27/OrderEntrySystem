#pragma once
#include "price_level.hpp"
#include <map>
#include <unordered_map>
#include <mutex>
#include <functional>
#include <cstddef>
#include <string>
#include <cstdint>

class OrderBook {
private:
    // recursive: trade notifications may snapshot the book while matching holds the lock
    mutable std::recursive_mutex book_mutex;
    // Balanced BST for bid side (descending order - highest price first)
    std::map<Price, PriceLevel, std::greater<Price>> bids;
    
    // Balanced BST for ask side (ascending order - lowest price first)
    std::map<Price, PriceLevel, std::less<Price>> asks;
    
    // Order map for O(1) lookup by OrderID
    std::unordered_map<OrderID, Order*> order_map;
    
    // Top of book cache (optional optimization)
    Price best_bid_price = 0;          // 0 means no bids
    Price best_ask_price = NO_ASK;  // sentinel: no asks
    
    // Helper methods
    void updateBestBid();
    void updateBestAsk();
    void removeEmptyPriceLevel(Side side, Price price);
    
public:
    OrderBook() = default;
    
    // Core operations
    bool addOrder(Order* order, std::function<void(OrderID, Price, Quantity)> onTradeExecution);
    bool cancelOrder(OrderID id);
    
    // Matching (to be implemented later with matching engine)
    void match(Order* incoming_order, std::function<void(OrderID, Price, Quantity)> onTradeExecution);
    
    // Accessors
    Price getBestBid() const;
    Price getBestAsk() const;
    Quantity getVolumeAtPrice(Side side, Price price) const;
    Order* getOrder(OrderID id) const;
    
    // Check if order book can match (best bid >= best ask)
    bool canMatch() const;
    
    // For debugging/display
    void printBook() const;
    size_t getBidLevelCount() const { return bids.size(); }
    size_t getAskLevelCount() const { return asks.size(); }

    // JSON snapshot for UI: top N price levels each side + top-of-book cache fields
    std::string snapshot_top_json(std::size_t max_levels, std::uint32_t connections) const;
};