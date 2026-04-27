import asyncio
import websockets
import json
import random
import time
import csv
import numpy as np

URI = "ws://localhost:8080/ws"

class BaseAgent:
    def __init__(self, name):
        self.name = name
        self.ws = None
        self.running = True

    async def connect(self):
        try:
            # 1. Stagger the initial connections randomly by 0 to 1 second
            # This prevents overwhelming the C++ server's connection queue on boot
            await asyncio.sleep(random.uniform(0.0, 1.0))
            
            # 2. Disable ping timeouts. During the Phase 2 HFT flood, the TCP 
            # buffers get totally saturated. This tells Python not to panic and 
            # drop the connection if the C++ server is delayed in responding.
            self.ws = await websockets.connect(
                URI, 
                ping_interval=None, 
                ping_timeout=None
            )
            
            asyncio.create_task(self.listen())
            asyncio.create_task(self.behavior_loop())
        except Exception as e:
            print(f"[{self.name}] Connection failed: {e}")

    async def listen(self):
        try:
            async for message in self.ws:
                try:
                    # Attempt to parse the message as JSON (for telemetry)
                    data = json.loads(message)
                    await self.on_message(data)
                except json.JSONDecodeError:
                    # The C++ server sent a plain text status update instead of JSON.
                    # We simply pass/ignore it so the bot doesn't crash.
                    pass
        except websockets.ConnectionClosed:
            pass

    async def send_order(self, action, order_type, side, price, qty):
        if not self.ws: return
        # Assuming your C++ backend expects integer prices (e.g., $10.00 -> 1000)
        payload = {
            "action": action,
            "type": order_type,
            "side": side,
            "price": int(price),
            "qty": int(qty)
        }
        await self.ws.send(json.dumps(payload))

    async def on_message(self, data):
        pass

    async def behavior_loop(self):
        pass

    async def stop(self):
        self.running = False
        if self.ws:
            await self.ws.close()


class DataLogger(BaseAgent):
    def __init__(self, name, filename="fifo_telemetry.csv"):
        super().__init__(name)
        self.stability_values = []
        # Open CSV to record the time series
        self.csv_file = open('stability_bounds.csv', 'w', newline='')
        self.csv_writer = csv.writer(self.csv_file)
        self.csv_writer.writerow(['timestamp', 'S', 'L_eff', 'H_c', 'H_p'])

    async def on_message(self, data):
        # Listen exclusively for the telemetry payload from the 100ms C++ timer
        if data.get("type") == "telemetry":
            s_val = data.get("S", 0.0)
            self.stability_values.append(s_val)
            
            # Log to CSV for graphing later
            self.csv_writer.writerow([
                data.get("timestamp"), 
                s_val, 
                data.get("L_eff"), 
                data.get("H_c"), 
                data.get("H_p")
            ])

    def close(self):
        self.csv_file.close()


class MarketMaker(BaseAgent):
    def __init__(self, name, anchor_price):
        super().__init__(name)
        self.anchor_price = anchor_price  # e.g., 1000 for $10.00
        self.panicking = False

    async def trigger_panic(self):
        """Simulates adverse selection avoidance by scrubbing all quotes."""
        if not self.panicking:
            self.panicking = True
            print(f"[{self.name}] Adverse selection detected! Scrubbing quotes...")
            # Fire the massive cancel avalanche to pull all resting liquidity
            await self.send_order("cancel_all", "LIMIT", "BUY", 0, 0)
            await self.send_order("cancel_all", "LIMIT", "SELL", 0, 0)

    async def behavior_loop(self):
        # Establish a massive, stable queue for S_max
        await asyncio.sleep(1) # Let system boot
        print(f"[{self.name}] Establishing deep quotes...")
        await self.send_order("new_order", "LIMIT", "BUY", self.anchor_price - 5, 50000)
        await self.send_order("new_order", "LIMIT", "SELL", self.anchor_price + 5, 50000)
        
        while self.running:
            # Periodically refresh quotes slowly to simulate passive liquidity
            if self.panicking:
                await asyncio.sleep(5)
            await self.send_order("new_order", "LIMIT", "BUY", self.anchor_price - 5, 1000)


class RetailTrader(BaseAgent):
    async def behavior_loop(self):
        while self.running:
            # Infrequent, random market orders to generate baseline trade volume
            await asyncio.sleep(random.uniform(0.5, 2.0))
            side = random.choice(["BUY", "SELL"])
            qty = random.randint(10, 100)
            # Market orders don't care about price, but we pass 0 as a placeholder
            await self.send_order("new_order", "MARKET", side, 0, qty)


class HFTSniper(BaseAgent):
    def __init__(self, name, anchor_price):
        super().__init__(name)
        self.anchor_price = anchor_price
        self.current_bid = anchor_price - 4
        self.current_ask = anchor_price + 4

    async def behavior_loop(self):
        print(f"[{self.name}] Starting aggressive pennying algorithm.")
        while self.running:
            # Hyper-aggressive loop: constantly cancel and step ahead by 1 tick
            # This is designed to break pure FIFO and spike H_p and H_c
            await asyncio.sleep(0.05) # 50ms latency
            
            # Step inside the spread
            self.current_bid += 1
            self.current_ask -= 1

            # Reset if we cross the spread
            if self.current_bid >= self.current_ask:
                self.current_bid = self.anchor_price - 4
                self.current_ask = self.anchor_price + 4
                # Simulating a massive cancel avalanche
                await self.send_order("cancel_all", "LIMIT", "BUY", 0, 0)

            # Send tiny orders (minimum liquidity) but at new price levels (maximum heat)
            await self.send_order("new_order", "LIMIT", "BUY", self.current_bid, 1)
            await self.send_order("new_order", "LIMIT", "SELL", self.current_ask, 1)


async def main():
    anchor_price = 1000 # $10.00
    
    logger = DataLogger("Telemetry")
    makers = [MarketMaker(f"MM_{i}", anchor_price) for i in range(3)]
    retail = [RetailTrader(f"Retail_{i}") for i in range(2)]
    snipers = [HFTSniper(f"HFT_{i}", anchor_price) for i in range(5)]

    # Connect the passive/baseline agents
    agents = [logger] + makers + retail
    await asyncio.gather(*(agent.connect() for agent in agents))

    print("\n=== PHASE 1: MAX STABILITY ===")
    print("Market Makers are building deep book. Expect high S.")
    await asyncio.sleep(15) 

    print("\n=== PHASE 2: MIN STABILITY ===")
    print("Unleashing HFT Snipers to penny the spread. Expect S to collapse.")
    for maker in makers:
        await maker.trigger_panic()
        
    for sniper in snipers:
        await sniper.connect()
        agents.append(sniper)
    
    await asyncio.sleep(15)

    print("\n=== SIMULATION COMPLETE ===")

    logger.close()
    
    # Calculate the bounds from the collected data
    s_array = np.array(logger.stability_values)
    
    if len(s_array) > 0:
        # We use percentiles instead of strictly min/max to filter out 
        # the initial 0.0 values from when the book is completely empty, 
        # or massive edge-case spikes.
        s_min = np.percentile(s_array, 5)   # Bottom 5%
        s_max = np.percentile(s_array, 95)  # Top 5%
        s_median = np.median(s_array)
        
        print(f"Total Telemetry Ticks Logged: {len(s_array)}")
        print(f"Empirical S_min (5th percentile):  {s_min:.4f}")
        print(f"Empirical S_max (95th percentile): {s_max:.4f}")
        print(f"Median S value:                    {s_median:.4f}")
        
        print("\nYou can now use these bounds to map your exponential function!")
        print("Example: f(S) = a * exp(b * S) + c")
        print(f"Where f({s_min:.4f}) maps to 50% FIFO, and f({s_max:.4f}) maps to 100% FIFO.")
    else:
        print("Error: No telemetry data collected. Check WebSocket connection.")

if __name__ == "__main__":
    asyncio.run(main())