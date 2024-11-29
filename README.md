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

## Dependencies

**lumactl** doesn't have any build time dependency, other than a working Rust compiler and
toolchain. At runtime, it uses [_wmctl_](https://github.com/danyspin97/wmctl) to get the current
available displays, if the display argument hasn't been passed to it. _wmctl_ currently only
supports Wayland, but **lumactl** works with any display as long as you hard-code its name (i.e.
`DP-4`, `eDP-1` or `HDMI-A-1`, refer to your current window manager documentation on how
to get the mentioned information).

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
