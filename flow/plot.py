import serial
import matplotlib.pyplot as plt
import matplotlib.animation as animation
import time

# Serial port setup
serial_port = "/dev/cu.usbserial-210"
baud_rate = 115200
ser = serial.Serial(serial_port, baud_rate, timeout=1)

# Data storage
x_data = []
y_data = []
start_time = time.time()

# Plot setup
fig, ax = plt.subplots()
(line,) = ax.plot([], [], label="Flow Rate (L/min)")
ax.set_xlabel("Time (s)")
ax.set_ylabel("Flow Rate (L/min)")
ax.legend(loc="upper right")
ax.set_xlim(0, 10)
ax.set_ylim(0, 30)  # Adjust as per your flow rate range


def update(frame):
    line_data = ser.readline().decode("utf-8").strip()
    if line_data:
        try:
            flow_rate = float(line_data)
            elapsed_time = time.time() - start_time
            x_data.append(elapsed_time)
            y_data.append(flow_rate)

            # Keep only the last N data points
            N = 100
            x_data_plot = x_data[-N:]
            y_data_plot = y_data[-N:]

            line.set_data(x_data_plot, y_data_plot)
            ax.set_xlim(max(0, elapsed_time - 10), elapsed_time)
            return (line,)
        except ValueError:
            pass
    return (line,)


ani = animation.FuncAnimation(
    fig,
    update,
    blit=True,
    interval=10,
)

plt.show()
