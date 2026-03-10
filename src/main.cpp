#include "order_book.hpp"
#include <iostream>

int main() {
    OrderBook book;
    
    // Create some orders (in production, these come from object pool)
    Order* o1 = new Order{1, Side::BUY, 10050, 100};   // Buy 100 @ $100.50
    Order* o2 = new Order{2, Side::BUY, 10040, 200};   // Buy 200 @ $100.40
    Order* o3 = new Order{3, Side::SELL, 10060, 150};  // Sell 150 @ $100.60
    Order* o4 = new Order{4, Side::SELL, 10070, 100};  // Sell 100 @ $100.70
    
    book.addOrder(o1);
    book.addOrder(o2);
    book.addOrder(o3);
    book.addOrder(o4);
    
    book.printBook();
    
    std::cout << "Best Bid: $" << book.getBestBid() / 100.0 << std::endl;
    std::cout << "Best Ask: $" << book.getBestAsk() / 100.0 << std::endl;
    
    // Cancel an order
    book.cancelOrder(2);
    std::cout << "\nAfter canceling order 2:" << std::endl;
    book.printBook();
    
    return 0;
}