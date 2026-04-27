import pandas as pd
import matplotlib.pyplot as plt

# Load the data
df = pd.read_csv("stability_bounds.csv")
df['time_sec'] = (df['timestamp'] - df['timestamp'].iloc[0]) / 1000.0

# Create a 3-panel plot
fig, (ax1, ax2, ax3) = plt.subplots(3, 1, figsize=(10, 10), sharex=True)

# Plot Stability (S)
ax1.plot(df['time_sec'], df['S'], color='cyan')
ax1.set_title("Order Book Stability (S)")
ax1.set_ylabel("S")
ax1.grid(True, alpha=0.3)

# Plot Effective Liquidity (L_eff)
ax2.plot(df['time_sec'], df['L_eff'], color='lightgreen')
ax2.set_title("Effective Liquidity (L_eff)")
ax2.set_ylabel("Volume")
ax2.grid(True, alpha=0.3)

# Plot Heat (H_c and H_p)
ax3.plot(df['time_sec'], df['H_c'], label="Cancel Heat (H_c)", color='red', alpha=0.7)
ax3.plot(df['time_sec'], df['H_p'], label="Price Heat (H_p)", color='orange', alpha=0.7)
ax3.set_title("Market Heat Variables")
ax3.set_xlabel("Time (seconds)")
ax3.set_ylabel("Heat")
ax3.legend()
ax3.grid(True, alpha=0.3)

plt.tight_layout()
plt.show()