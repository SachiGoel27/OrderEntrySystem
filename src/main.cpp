#include "crow_all.h"
#include "order_book.hpp"
#include <iostream>
#include <string>
#include <atomic>
#include <functional>
#include <mutex>
#include <set>

// Store active connections to broadcast trade reports
std::mutex connections_mutex;
std::set<crow::websocket::connection*> active_connections;

int main() {
    crow::SimpleApp app;
    OrderBook book; 
    
    // std::atomic ensures that even if multiple threads ask for an ID at the 
    // exact same nanosecond, they are guaranteed to get unique, sequential numbers.
    std::atomic<uint64_t> global_order_id{1}; 

    // Create a WebSocket route at ws://localhost:8080/ws
    CROW_WEBSOCKET_ROUTE(app, "/ws")
      .onopen([&](crow::websocket::connection& conn) {
          std::lock_guard<std::mutex> lock(connections_mutex);
          active_connections.insert(&conn);
          std::cout << "New trader connected!" << std::endl;
      })
      .onclose([&](crow::websocket::connection& conn, const std::string& reason, uint16_t code) {
          std::lock_guard<std::mutex> lock(connections_mutex);
          active_connections.erase(&conn);
          std::cout << "Trader disconnected." << std::endl;
      })
      .onmessage([&](crow::websocket::connection& conn, const std::string& data, bool is_binary) {
          // 1. Parse the incoming JSON message
          auto incoming_json = crow::json::load(data);
          if (!incoming_json) {
              conn.send_text("Error: Invalid JSON format");
              return;
          }

          if (incoming_json.has("action") && incoming_json["action"].s() == "cancel") {
              if (incoming_json.has("id")) {
                  OrderID id = incoming_json["id"].i();
                  bool success = book.cancelOrder(id);
                  if (success) {
                      conn.send_text("Success: Order " + std::to_string(id) + " cancelled.");
                  } else {
                      conn.send_text("Error: Failed to cancel order " + std::to_string(id) + ".");
                  }
              }
              return;
          }

          // 2. Extract values (Expecting: {"side": "BUY", "price": 10050, "qty": 100})
          try {
              std::string side_str = incoming_json["side"].s();
              Price price = incoming_json.has("price") ? incoming_json["price"].i() : 0;
              Quantity qty = incoming_json["qty"].i();
              
              std::string type_str = "LIMIT";
              if (incoming_json.has("type")) type_str = incoming_json["type"].s();
              
              std::string tif_str = "GTC";
              if (incoming_json.has("tif")) tif_str = incoming_json["tif"].s();

              Side side = (side_str == "BUY" || side_str == "buy") ? Side::BUY : Side::SELL;
              OrderType type = (type_str == "MARKET" || type_str == "market") ? OrderType::MARKET : OrderType::LIMIT;
              
              TimeInForce tif = TimeInForce::GTC;
              if (tif_str == "IOC" || tif_str == "ioc") tif = TimeInForce::IOC;
              if (tif_str == "FOK" || tif_str == "fok") tif = TimeInForce::FOK;
              
              uint64_t id = global_order_id.fetch_add(1);

              // 3. Create the order using object pool
              Order* o = OrderPool::getInstance().acquire();
              o->id = id;
              o->side = side;
              o->type = type;
              o->tif = tif;
              o->price = price;
              o->qty = qty;

              // 4. Add to the order book and execute match instantly
              auto trade_callback = [](OrderID order_id, Price match_price, Quantity fill_qty) {
                  std::string report = "Trade Execution: Order " + std::to_string(order_id) + " filled " + std::to_string(fill_qty) + " @ $" + std::to_string(match_price / 100.0);
                  std::lock_guard<std::mutex> lock(connections_mutex);
                  for (auto* current_conn : active_connections) {
                      current_conn->send_text(report);
                  }
              };
              
              bool success = book.addOrder(o, trade_callback);

              if (success) {
                  conn.send_text("Success: Order " + std::to_string(id) + " processed.");
                  if (o->qty == 0 || o->type == OrderType::MARKET || o->tif == TimeInForce::IOC) {
                      // Handled immediately, no need to rest
                      if (o->qty > 0) {
                          conn.send_text("Status: Order " + std::to_string(id) + " cancelled remaining " + std::to_string(o->qty) + " (IOC/Market)");
                      }
                      OrderPool::getInstance().release(o);
                  }
              } else {
                  conn.send_text("Error: Failed to trace order or FOK condition not met.");
                  OrderPool::getInstance().release(o); 
              }
          } catch (const std::exception& e) {
              conn.send_text("Error: Missing required fields (side, price, qty)");
          }
      });

    std::cout << "Matching Engine starting on port 8080..." << std::endl;
    
    // Run the server with 4 concurrent network threads
    app.port(8080).multithreaded().run();
}