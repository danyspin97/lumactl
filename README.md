# Lumactl

A daemon to control the brightness on Linux. It supports both backlight brightness as well as
external displays controlled with DDC protocol. The scope is unifying both interfaces under
one tool, making it possible to use as bridge for other tools like keybindings and bars.

## Features

- Supports backlight brightness
- Supports for external monitors via DDC
- Easy to use command line interface
- Supports for relative increase/decreases
- Designed to be fast (hence the client/daemon interface)

## Getting started

To build **lumactl** local, run:

```bash
$ cargo build --release
```

Then execute the daemon in the background via:

```bash
$ lumactld --daemon
```

You can control the brightness by calling **lumactl** directly:

```bash
# Get the brightness in percentage for all displays
$ lumactl get --percentage
# Set the brightness to 100 for all displays
$ lumactl set 100
# Decrease the brightness for display DP-4 by 20%
$ lumactl set --display DP-4 -20%
```
 
## Why is lumactld a daemon
The first iteration of this small tool was indeed a simple command line probing all available
DDC interfaces via `ddc_hi` crate. However, probing all of them takes 2 seconds or more on my
workstation, which means that **lumactl** took from 2 to 3 seconds every time it runs.
This is definitely not acceptable for integration into any other tool, which should expect a
result in a reasonable time. The daemon will enumerate and probe all DDC interfaces only at
startup and when a display gets connected or disconnected, making **lumactl** just send the
command as quickly as possible. It also leaves open the opportunity to further optimizations
in the future like caching the current brightness or using inotify for backlight brightness.

## License

**lumactl** is licensed under the GPL-3.0+ license.
