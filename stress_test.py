import asyncio
import websockets
import json
import time

async def spam_orders(uri, thread_id):
    """Spams 10,000 orders as fast as possible from a single connection."""
    try:
        async with websockets.connect(uri) as websocket:
            for i in range(10000):
                # Alternate between buy and sell to mix up the tree traversal
                side = "BUY" if i % 2 == 0 else "SELL"
                order = {
                    "side": side,
                    "price": 10000 + (i % 100), # Spread prices across 100 ticks
                    "qty": 10
                }
                await websocket.send(json.dumps(order))
                
                # Optional: wait for response, or just blind-fire
                # response = await websocket.recv() 
    except Exception as e:
        print(f"Thread {thread_id} failed: {e}")

async def main():
    uri = "ws://localhost:8080/ws"
    print("Starting stress test...")
    start_time = time.time()
    
    # Create 20 concurrent network connections, all firing simultaneously
    tasks = [spam_orders(uri, i) for i in range(20)]
    
    # Run them all at once
    await asyncio.gather(*tasks)
    
    print(f"Test completed in {time.time() - start_time:.2f} seconds.")

if __name__ == "__main__":
    asyncio.run(main())