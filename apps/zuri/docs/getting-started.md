# Installing Zuric

Zuric comes prepackaged with all Zuri distributions and needs no further installation action. You'll need to upgrade your Zuri installation you are on a Zuri version lower than `v0.1.0`.

The best way to enjoy the power of Zuri is to add your Zuri installation path to your shell/terminal environment. Various operating systems provide different mechanisms for adding a path to the environment, so the steps may vary for your specific operating system.

Here are a few links for different operating systems showing how to do this.

| Operating systems | Instruction Link                                                                                                                                   |
|-------------------|----------------------------------------------------------------------------------------------------------------------------------------------------|
| Linux, macOS      | [https://opensource.com/article/17/6/set-path-linux](https://opensource.com/article/17/6/set-path-linux)                                           |
| Windows           | [https://www.wikihow.com/Change-the-PATH-Environment-Variable-on-Windows](https://www.wikihow.com/Change-the-PATH-Environment-Variable-on-Windows) |


### Testing your installation

If you have installed Zuri and successfully added Zuri installation directory to a system path, open a new terminal session (this may be required) and run the command `zuri --version`.

You should see an output similar to the below.

```
Zuri 0.1.0 (running on ZuriVM 0.1.0)
```

You can also run the `zuri` command without any arguments to see the full help information.

```
Usage: zuri [-v] [-h] [COMMAND] 
 
OPTIONS: 
  -h, --help     Show this help message and exit 
  -v, --version  Show Zuri version 
 
COMMANDS: 
  account <VALUE>      Manages a package publisher account 
  bundle               Creates a standalone application bundle. 
  clean                Clear Zuri repository storage and cache 
  fetch <VALUE>        Fetch a package from a Git repository 
  info                 Shows current project information 
  init                 Creates a new package in current directory 
  install <VALUE>      Installs a Zuri package 
  publish              Publishes a Zuri package to a repository 
  restore              Restores all project dependencies 
  run <VALUE>          Run a zuri project or script 
  serve                Starts a local Zuric repository server 
  test                 Run the tests 
  uninstall <VALUE>    Uninstalls a Zuri package 
 
Run "zuri --help [COMMAND]" for help on a specific command. 
```

If you can see this, then you're all good.
