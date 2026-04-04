#include "crow_all.h"
#include "order_book.hpp"
#include <iostream>
#include <string>
#include <atomic>

int main() {
    crow::SimpleApp app;
    OrderBook book; 
    
    // std::atomic ensures that even if multiple threads ask for an ID at the 
    // exact same nanosecond, they are guaranteed to get unique, sequential numbers.
    std::atomic<uint64_t> global_order_id{1}; 

    // Create a WebSocket route at ws://localhost:8080/ws
    CROW_WEBSOCKET_ROUTE(app, "/ws")
      .onopen([&](crow::websocket::connection& conn) {
          std::cout << "New trader connected!" << std::endl;
      })
      .onclose([&](crow::websocket::connection& conn, const std::string& reason, uint16_t code) {
          std::cout << "Trader disconnected." << std::endl;
      })
      .onmessage([&](crow::websocket::connection& conn, const std::string& data, bool is_binary) {
          // 1. Parse the incoming JSON message
          auto incoming_json = crow::json::load(data);
          if (!incoming_json) {
              conn.send_text("Error: Invalid JSON format");
              return;
          }

          // 2. Extract values (Expecting: {"side": "BUY", "price": 10050, "qty": 100})
          try {
              std::string side_str = incoming_json["side"].s();
              Price price = incoming_json["price"].i();
              Quantity qty = incoming_json["qty"].i();

              Side side = (side_str == "BUY" || side_str == "buy") ? Side::BUY : Side::SELL;
              uint64_t id = global_order_id.fetch_add(1);

              // 3. Create the order
              Order* o = new Order{id, side, price, qty};

              // 4. Add to the order book (THIS IS WHERE THE RACE CONDITION WILL HAPPEN)
              bool success = book.addOrder(o);

              if (success) {
                  conn.send_text("Success: Order " + std::to_string(id) + " added.");
              } else {
                  conn.send_text("Error: Failed to add order.");
                  delete o; 
              }
          } catch (const std::exception& e) {
              conn.send_text("Error: Missing required fields (side, price, qty)");
          }
      });

    std::cout << "Matching Engine starting on port 8080..." << std::endl;
    
    // Run the server with 4 concurrent network threads
    app.port(8080).multithreaded().run();
}