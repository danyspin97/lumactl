# Lumactl

**lumactl** is a tool to control the brightness on Linux. It supports both backlight brightness and
external displays controlled with DDC protocol. The scope is unifying both interfaces under
one tool, making it possible to use as interface for other tools like window managers and bars.

## Features

- Supports backlight brightness
- Supports for external monitors via DDC
- Easy to use command line interface
- Supports for relative increase/decreases
- Designed to be fast

## Getting started

To build **lumactl** local, run:

```bash
$ cargo build --release
```

You can control the brightness by calling **lumactl**:

```bash
# Get the brightness in percentage for all displays
$ lumactl get --percentage
# Set the brightness to 100 for all displays
$ lumactl set 100
# Decrease the brightness for display DP-4 by 20%
$ lumactl set --display DP-4 -20%
```
 
## License

**lumactl** is licensed under the GPL-3.0+ license.
