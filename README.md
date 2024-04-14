# Little Big Mouse

<p align="center">
    <img src="https://raw.githubusercontent.com/mgth/LittleBigMouse/master/LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/lbm.png" width="200" alt="Little Big Mouse Logo"/>
</p>
Little Big Mouse (LBM) is an open-source software designed to enhance the multi-monitor experience on Windows 10 and 11 by providing accurate mouse screen crossover location within multi-DPI monitors environments. This is particularly useful for setups involving a 4K monitor alongside a full HD monitor.

## Features

- **Smooth Mouse Transitions**: Ensures smooth and accurate mouse movement across screens with different DPI settings.
- **DPI Aware Mouse Movement**: Adjusts mouse speed to remain consistent across monitors with different DPI values.
- **Infinite Mouse Scrolling**: Enables seamless cursor movement between screens, either horizontally or vertically.
- **Display Size Adjustments**: Allows for adjustments in the relative sizes of displays.
- **Display Color and Brightness Balancing**: Offers control over color and brightness profiles of displays.
- **Access to Display Debugging Information**: Provides detailed information from your displays and drivers.

## Installation

1. Download the latest release from the [Releases](https://github.com/mgth/LittleBigMouse/releases) page.
2. Run the Little Big Mouse installer from your computer.
3. (Optional) Change the default installation path.
4. Follow the installation progress and complete the installation.
5. Access the program from the Start Menu.

## Usage

Little Big Mouse provides a single-window interface with three main sections:

- **Top Panel**: Access view tabs for display and display adapter information, changing relative sizes and positions of displays, and adjusting color and brightness profiles.
- **Center Panel**: Displays information about your display devices, including makes and models, capabilities, adapters, and relative positions.
- **Bottom Panel**: Offers options and operations, including copying config to clipboard, enabling/disabling LBM functionality, and adjusting speed, pointer size, and corner crossing.

### Key Features

- **View Display Info**: Access detailed information from your displays and drivers.
- **Change Display Sizes**: Correct incorrect dimensions reported by your display driver.
- **Change Display Positions**: Define the physical locations of each display for smooth mouse transitions.
- **Change Display Color**: Adjust color profiles for better color matching between displays.

## Command Line

You can use the command line to start/stop Little Big Mouse:

- To start: `<install_directory>\LittleBigMouse_Daemon.exe --start`
- To stop: `<install_directory>\LittleBigMouse_Daemon.exe --stop`

## Support

If you encounter any issues or have suggestions for improvements, please open an issue on our [Issues](https://github.com/mgth/LittleBigMouse/issues) page. We appreciate your feedback and are always looking to improve the tool.

**We Welcome Contributions**: Your help is invaluable! Whether it's reporting bugs, suggesting new features, or submitting pull requests, we encourage you to get involved. Your contributions can make a significant difference in the development and improvement of Little Big Mouse. **First time?** Check out [This guide](https://github.com/firstcontributions/first-contributions) to get started.

## Donations

If you find Little Big Mouse helpful, consider supporting the project with a donation. Your contribution helps us maintain and improve the software.

[![Donate](https://img.shields.io/badge/Donate-PayPal-green.svg)](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=YLGYPSHWTQ5UW&lc=FR&item_name=Mgth&currency_code=EUR&bn=PP%2dDonationsBF%3abtn_donateCC_LG%2egif%3aNonHosted)

## Acknowledgements

- **HLab Project**: Little Big Mouse depends on the HLab project for its functionality. Check out the [HLab](https://github.com/CHMP-HLab/HLab) and [HLab.Avalonia](https://github.com/mgth/HLab.Avalonia) repositories for more information.
- **MouseKeyboardActivityMonitor**: Inspired by the [MouseKeyboardActivityMonitor](https://github.com/gmamaladze/globalmousekeyhook) project.
- **Task Scheduler Managed Wrapper**: Utilizes the [Task Scheduler Managed Wrapper](https://github.com/dahall/TaskScheduler) for scheduling tasks.

Thank you for using Little Big Mouse!
