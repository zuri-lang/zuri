#!/bin/bash

set -u

abort() {
  printf "%s\n" "$@"
  exit 1
}

# Fail fast with a concise message when not using bash
# Single brackets are needed here for POSIX compatibility
# shellcheck disable=SC2292
if [ -z "${BASH_VERSION:-}" ]; then
  abort "Bash is required to interpret this script."
fi

PROFILE_FILE="$HOME/.profile"

# Check OS.
OS="$(uname)"
if [[ "${OS}" == "Linux" ]]; then
  IS_LINUX=1
elif [[ "${OS}" != "Darwin" ]]; then
  abort "Zuri auto install is only supported on macOS and Linux."
fi

install_if_missing() {
  for value in "$@"
  do
    if [[ $(command -v "$value") == "" ]]; then
      echo "$value is not installed. Attempting to install it!"

      if [[ "$value" == "cmake" && -x "$(command -v snap)" ]]; then
        # On Ubuntu with snap, snap is the correct way to get an up-to-date cmake version.
        snap install cmake --classic
      elif [ -x "$(command -v apk)" ]; then
        sudo apk add --no-cache "$value"
      elif [ -x "$(command -v apt-get)" ]; then
        sudo apt-get install "$value" -y
      elif [ -x "$(command -v dnf)" ]; then
        sudo dnf install "$value" -y
      elif [ -x "$(command -v zypper)" ]; then
        sudo zypper install "$value" -y
      elif [ -x "$(command -v yum)" ]; then
        sudo yum install "$value" -y
      elif [ -x "$(command -v pacman)" ]; then
        sudo pacman -Sy "$value"
      elif [ -x "$(command -v brew)" ]; then
        brew install "$value"
      else
        echo "Failed to install dependencies. Package manager not found."
        abort "You must manually install $value to continue"
      fi
    else
      echo "$value is already installed. Skipping..."
    fi
  done
}

install_build_env() {
  # shellcheck disable=SC2154
  if [[ $(command -v "make") == "" ]]; then
    echo "Build environment is not setup. Setting it up"

    if [ -x "$(command -v apt-get)" ]; then
      sudo apt-get install build-essential -y
    elif [ -x "$(command -v apk)" ]; then
      sudo apk add --no-cache build-base
    elif [ -x "$(command -v dnf)" ]; then
      sudo dnf group install "C Development Tools and Libraries" -y
    elif [ -x "$(command -v zypper)" ]; then
      sudo zypper install -t pattern devel_basis -y
    elif [ -x "$(command -v yum)" ]; then
      sudo yum groups mark install "Development Tools" -y
      sudo yum groups mark convert "Development Tools" -y
      sudo yum groupinstall "Development Tools" -y
    elif [ -x "$(command -v pacman)" ]; then
      sudo pacman -Sy base-devel
    else
      echo "Failed to install dependencies. Package manager not found."
      abort "You must manually setup your build environment by installing make, c/c++ and related modules to continue"
    fi
  else
    echo "Build environment is already setup. Skipping..."
  fi
}

install_platform_specific_tools() {
  # shellcheck disable=SC2154
  if [ -x "$(command -v apt-get)" ]; then
    sudo apt-get install libpthread-stubs0-dev -y
  elif [ -x "$(command -v apk)" ]; then
    sudo apk add --no-cache glibc-headers
  elif [ -x "$(command -v dnf)" ]; then
    sudo dnf install glibc-headers -y
  elif [ -x "$(command -v zypper)" ]; then
    sudo zypper install glibc-devel -y
  elif [ -x "$(command -v yum)" ]; then
    sudo yum install glibc-headers -y
  elif [ -x "$(command -v pacman)" ]; then
    sudo pacman -Sy glibc
  fi
}

remove_redundant_libraries() {
  # shellcheck disable=SC2154
  if [ -x "$(command -v apt-get)" ]; then
    sudo apt-get remove libgd3 -y
  elif [ -x "$(command -v brew)" ]; then
    brew uninstall pkg-config --ignore-dependencies
  elif [ -x "$(command -v apk)" ]; then
    sudo apk delete libgd3 -y
  fi
}

install_zuri() {
  if [[ -d "$1" ]]; then
    sudo mkdir -p "$1"
  fi

	# removing old/stale/partial objects
	STALE_PATHS=("$(pwd)/zuri" "$1/.zuri")
	for path in "${STALE_PATHS[@]}"
	do
	  if [[ -d "$path" ]]; then
	    echo "Removing old/stale/partial object '$path'..."
      sudo rm -rf "$path" || exit
    fi
  done

	# cloning
	git clone https://github.com/zuri-lang/zuri.git
#	git checkout v0.0.86
	cd zuri || exit

	# building
	if [[ "${OS}" == "Darwin" ]]; then
	  cmake -B . -DOPENSSL_ROOT_DIR=/opt/homebrew/opt/openssl/ || exit
  else
	  cmake -B . || exit
  fi
	cmake --build . -- -j 16 || exit

	# We are copying to .zuri here instead of just moving it to
	# zuri directly in case the user runs this script from the
	# home directory.
	cp -r zuri "$1/.zuri"

	cd ..
	sudo rm -rf zuri

  ADD_TO_PATH="export PATH=\$PATH:\"$1/.zuri\""

	if ! grep -q "$ADD_TO_PATH" "$PROFILE_FILE"; then
    # Now add the zuri executable to path
    echo "Adding Zuri to path..."

    # export to bash profile
    echo "$ADD_TO_PATH" | sudo tee -a "$PROFILE_FILE" > /dev/null

    # make available in current session
    exec "$SHELL" -l;
  fi

  echo "Zuri installed successfully!"
}

echo "Beginning installation of Zuri..."

install_build_env
remove_redundant_libraries
install_platform_specific_tools

if [[ -z "${IS_LINUX-}" ]]; then

  #Install Homebrew if not installed...
  if [[ $(command -v 'brew') == "" ]]
  then
    echo "Homebrew not installed. Installing..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  else
    echo "Brew is installed. Using brew!"
  fi

  install_if_missing 'pkg-config' 'openssl' 'gd' 'libffi'
else
  install_if_missing 'pkg-config' 'git' 'curl' 'libssl-dev' 'libcurl4-openssl-dev' 'libgd-dev' 'libavif-dev' 'libffi-dev'
  if [[ $(command -v 'apt') != "" ]]; then
    install_if_missing 'zlib1g-dev'
  else
    install_if_missing 'zlib-devel'
  fi
fi

#Install cmake dependency.
install_if_missing 'cmake'

if [[ "${OS}" == "Darwin" ]]; then
  install_zuri "/Users/$USER"
else
  install_zuri "/home/$USER"
fi
