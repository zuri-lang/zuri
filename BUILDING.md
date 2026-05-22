# Building Zuri

Zuri itself is a C11 application using the CMake build system, and therefore, building Zuri should be no different to
any other application built using the same tools.

## Automated Build and Install (Works on all OSX and Linux)

```shell
bash <(curl -s https://raw.githubusercontent.com/zuri-lang/zuri/main/scripts/install.sh)
```

## Prerequisites

### macOS

Make sure you have the Xcode CLT installed. The [Homebrew package manager](https://brew.sh) is the preferred way to
install dependencies on macOS.

```sh
sudo xcode-select    # prompt the user to install Xcode CLT if it is not already installed
sudo brew update
brew uninstall pkg-config --ignore-dependencies
brew install pkg-config gd libffi
```

Proceed to the [Configure](#configure) section to configure your CMake build.

### Linux

#### Debian, Ubuntu and their derivatives

Install the required dependencies using the `apt` package manager.

```sh
sudo apt update
sudo apt install build-essential curl libpthread-stubs0-dev libcurl4-openssl-dev libgd-dev libavif-dev libffi-dev
```

Proceed to the [Configure](#configure) section to configure your CMake build.

#### Arch Linux

Install the required dependencies using the `pacman` package manager.

```sh
sudo pacman -Sy
sudo pacman -S --needed --noconfirm base-devel openssl cmake pthread gd avif ffi curl
```

Proceed to the [Configure](#configure) section to configure your CMake build.

#### RedHat, Fedora, CentOS and their derivatives

Install the required dependencies using the `yum` package manager.

```sh
sudo yum check-update
sudo yum groupinstall 'Development Tools'
sudo yum install -y openssl-devel cmake glibc-devel gd-devel libffi-devel libavif curl libcurl-devel
```

Proceed to the [Configure](#configure) section to configure your CMake build.

### Windows

> Starting from the 3rd of February 2021, Zuri's officially supported compilers for the Windows environment is now the 
> WinLibs and TDM-GCC compiler. The decision to change the official compiler from Visual Studio and MSYS2 to WinLibs and TDM-GCC 
> is to allow for minial configuration effort while installing Zuri as well as to allow us to develop Zuri faster as trying to 
> be cross-compatible with Visual Studio has proven to slow down the growth of the language and the ecosystem and setting up 
> MSYS2 environment to compile Zuri is more work than required for either WinLibs or TDM-GCC.
> 
> This also allows us to build valid Zuri C extensions on Windows with less hassle. 
> Check out the [zuri-ext-demo](https://github.com/zuri-lang/zuri-ext-demo) or any of the extension in the 
> [packages](https://github.com/zuri-lang/zuri/packages) directory for more info on how to write a valid C extension for Zuri.

#### Using WinLibs or TDM-GCC with vcpkg

To install Zuri with WinLibs or TDM-GCC, install 
[WinLibs](https://github.com/brechtsanders/winlibs_mingw/releases/download/11.2.0-9.0.0-msvcrt-r5/winlibs-x86_64-posix-seh-gcc-11.2.0-mingw-w64-9.0.0-r5.zip) 
or [TDM-GCC](https://github.com/jmeubank/tdm-gcc/releases/download/v10.3.0-tdm64-2/tdm64-gcc-10.3.0-2.exe) 
via the given links. Add WinLibs or TDM-GCC `bin` directory to your environment path. TDM-GCC also allows you to add to path during its installation. 

Next, install [vcpkg](https://vcpkg.io/en/index.html) following the instruction [here](https://vcpkg.io/en/getting-started.html)
and add `vcpkg` to your environment. After this, run the commands below to install the required dependencies:

```shell
vcpkg install pkgconf pthreads:x64-windows curl:x64-windows libffi:x64-windows openssl:x64-windows libgd:x64-windows
```

If you are on an `x86` system, you can also install the x86 versions of the dependencies using the command:

```shell
vcpkg install pkgconf pthreads:x86-windows curl:x86-windows libffi:x86-windows openssl:x86-windows libgd:x86-windows
```

## Configure

Configure CMake by creating an empty `build` directory in the source root directory, and running the appropriate command.

> During configure, other required dependencies will be downloaded automatically the first time you do a configure so you should ensure you have a good internet connection if you are running configure for the first time.

### Unix (Linux, macOS)

```sh
cmake -B build
```

Optionally, if you have `ninja-build` installed, you may use `-G Ninja` to speed up compile times. For example:

```sh
cmake -B build -G Ninja
```

CMake will have generated a Unix Makefiles or Ninja build files in the `build` directory.

If you get an error regards missing `OPENSSL_ROOT_DIR` on macOS add the path to `openssl` which should be located at `/usr/local/opt/openssl` or somewhere in `/opt/homebrew/Cellar/` 
(for example `/opt/homebrew/Cellar/openssl@3/3.0.7`). Your configure command should look like this:

```sh
cmake -B build -DOPENSSL_ROOT_DIR=/usr/local/opt/openssl
```

### Windows

If you are configuring on Windows, you'll need to specify the `make` program as well as the `vcpkg` toolchain file.

```shell
cmake -B build -DCMAKE_MAKE_PROGRAM=mingw32-make -G "Unix Makefiles" -DCMAKE_TOOLCHAIN_FILE="${PATH_TO_VCPKG}/scripts/buildsystems/vcpkg.cmake" -DOPENSSL_ROOT_DIR="${PATH_TO_VCPKG}/installed/x64-windows-static"
```

On Windows, it is also mostly common to use the `ninja` build system when available. For this, you'll only need to replace the `mingw32-make` with `ninja` in the command.

## Build

Once the build files have been generated, either change to the `build` directory and run the appropriate `make` or 
`ninja` tool (the latter using the 'Developer Tools Command Prompt' on Windows), or ask CMake to call the correct 
tool for you:

```sh
cmake --build build
```

Zuri will be built into a directory called `zuri` which will be located in the `build` folder.
