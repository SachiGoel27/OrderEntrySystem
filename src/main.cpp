#include "crow_all.h"
#include "order_book.hpp"
#include <iostream>
#include <string>
#include <atomic>
#include <functional>
#include <mutex>
#include <set>
#include <sstream>

// Store active connections to broadcast trade reports
std::mutex connections_mutex;
std::set<crow::websocket::connection*> active_connections;

static void broadcast_text(const std::string& msg) {
    std::lock_guard<std::mutex> lock(connections_mutex);
    for (auto* current_conn : active_connections) {
        current_conn->send_text(msg);
    }
}

int main() {
    crow::SimpleApp app;
    OrderBook book;

    std::atomic<uint64_t> global_order_id{1};

    auto broadcast_book = [&]() {
        std::uint32_t n = 0;
        {
            std::lock_guard<std::mutex> lock(connections_mutex);
            n = static_cast<std::uint32_t>(active_connections.size());
        }
        std::string j = book.snapshot_top_json(10, n);
        broadcast_text(j);
    };

    auto broadcast_fill = [&](OrderID order_id, Price match_price, Quantity fill_qty) {
        std::ostringstream oss;
        oss << "{\"type\":\"fill\",\"order_id\":" << order_id << ",\"price\":" << match_price
            << ",\"qty\":" << fill_qty << "}";
        broadcast_text(oss.str());
    };

    CROW_WEBSOCKET_ROUTE(app, "/ws")
        .onopen([&](crow::websocket::connection& conn) {
            {
                std::lock_guard<std::mutex> lock(connections_mutex);
                active_connections.insert(&conn);
            }
            std::cout << "New trader connected!" << std::endl;
            broadcast_book();
        })
        .onclose([&](crow::websocket::connection& conn, const std::string& reason, uint16_t code) {
            std::lock_guard<std::mutex> lock(connections_mutex);
            active_connections.erase(&conn);
            std::cout << "Trader disconnected." << std::endl;
        })
        .onmessage([&](crow::websocket::connection& conn, const std::string& data, bool is_binary) {
            auto incoming_json = crow::json::load(data);
            if (!incoming_json) {
                conn.send_text("{\"type\":\"error\",\"message\":\"Invalid JSON format\"}");
                return;
            }

            if (incoming_json.has("type")) {
                try {
                    if (incoming_json["type"].s() == "client_ping") {
                        int64_t id = 0;
                        if (incoming_json.has("id")) {
                            id = incoming_json["id"].i();
                        }
                        std::ostringstream pong;
                        pong << "{\"type\":\"client_pong\",\"id\":" << id << "}";
                        conn.send_text(pong.str());
                        return;
                    }
                } catch (...) {
                    // "type" exists but is not a string (e.g. numeric); fall through to order handling
                }
            }

            if (incoming_json.has("action") && incoming_json["action"].s() == "cancel") {
                if (incoming_json.has("id")) {
                    OrderID id = static_cast<OrderID>(incoming_json["id"].i());
                    bool success = book.cancelOrder(id);
                    if (success) {
                        conn.send_text("{\"type\":\"cancel_result\",\"success\":true,\"id\":" + std::to_string(id) + "}");
                    } else {
                        conn.send_text("{\"type\":\"cancel_result\",\"success\":false,\"id\":" + std::to_string(id) + "}");
                    }
                    broadcast_book();
                }
                return;
            }

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

                Order* o = OrderPool::getInstance().acquire();
                o->id = id;
                o->side = side;
                o->type = type;
                o->tif = tif;
                o->price = price;
                o->qty = qty;

                auto trade_callback = [&](OrderID order_id, Price match_price, Quantity fill_qty) {
                    broadcast_fill(order_id, match_price, fill_qty);
                    broadcast_book();
                };

                bool success = book.addOrder(o, trade_callback);

                if (success) {
                    conn.send_text("{\"type\":\"ack\",\"order_id\":" + std::to_string(id) + ",\"status\":\"accepted\"}");
                    if (o->qty == 0 || o->type == OrderType::MARKET || o->tif == TimeInForce::IOC) {
                        if (o->qty > 0) {
                            conn.send_text("{\"type\":\"ack\",\"order_id\":" + std::to_string(id) +
                                           ",\"status\":\"ioc_market_remainder\",\"remaining_qty\":" +
                                           std::to_string(o->qty) + "}");
                        }
                        OrderPool::getInstance().release(o);
                    }
                } else {
                    conn.send_text("{\"type\":\"error\",\"message\":\"FOK not satisfied or addOrder failed\"}");
                    OrderPool::getInstance().release(o);
                }
                broadcast_book();
            } catch (const std::exception& e) {
                conn.send_text("{\"type\":\"error\",\"message\":\"Missing required fields (side, qty)\"}");
            }
        });

    std::cout << "Matching Engine starting on port 8080..." << std::endl;

    app.port(8080).multithreaded().run();
}
