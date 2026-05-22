# Installing Nyssa

Nyssa comes prepackaged with all Zuri versions since `v0.0.86` and needs no further installation action. You'll need to upgrade your Zuri installation you are on a Zuri version lower than `v0.0.86`.

The best way to enjoy the power of Nyssa is to add your Zuri installation path to your shell/terminal environment. Various operating systems provides different mechanisms for adding a path to the environment so the steps may vary for your specific operating system.

Here's a few links for different operating systems showing how to do this.

| Operating systems | Instruction Link |
|------------------|------------------|
| Linux, macOS | [https://opensource.com/article/17/6/set-path-linux](https://opensource.com/article/17/6/set-path-linux) |
| Windows | [https://www.wikihow.com/Change-the-PATH-Environment-Variable-on-Windows](https://www.wikihow.com/Change-the-PATH-Environment-Variable-on-Windows) |


### Testing your installation

If you have installed Zuri and successfully added Zuri installation directory to system path, open a new terminal session (may be required) and run the command `nyssa --version`.

You should see an output similar to the below.

```
Nyssa 0.2.0
Zuri 0.0.86 (running on ZuriVM 0.1.1)
```

You can also run the `nyssa` command without any arguments to see the full help information.

```
Usage: nyssa [ [-h] | [-v] ] [COMMAND]

OPTIONS:
  -h, --help                Show this help message and exit
  -v, --version             Show Nyssa version

COMMANDS:
  account <choice>          Manages a Nyssa publisher account
    create                    Creates a new publisher account
    login                     Login to a publisher account
    logout                    Log out of a publisher account
    -r, --repo <value>        the repo where the account is located
  clean                     Clear Nyssa storage and cache
    -c, --cache               clean packages cache
    -l, --logs                clean logs
    -a, --all                 clean everything
  info                      Shows current project information
  init                      Creates a new package in current directory
    -n, --name <value>        the name of the package
  install <value>           Installs a Zuri package
    -g, --global              installs the package globally
    -c, --use-cache           enables the cache
    -r, --repo <value>        the repository to install from
  publish                   Publishes a Zuri package to a repository
    -r, --repo <value>        repository url
  restore                   Restores all project dependencies
    -x, --no-cache            disables the cache
  serve                     Starts a local Nyssa repository server
    -p, --port <value>        port of the server (default: 3000)
    -n, --host <value>        the host ip (default: 127.0.0.1)
  test                      Run the tests
  uninstall <value>         Uninstalls a Zuri package
    -g, --global              package is a global package
```

If you can see this, then you're all good.
