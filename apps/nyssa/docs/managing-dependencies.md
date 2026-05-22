# Managing dependencies

As a package manager, one of the core responsibilities of nyssa is to help you manage dependencies for your application. In Zuri, there are three (3) scopes of libraries that exist.

- **Builtin libraries**: This libraries are the Zuri core libraries from which all other libraries are built on and are not meant to be managed by any third party application. Managing or modifying this packages yourself can break your installation and/or make your installation incompartible with other installations.
- **Global libraries**: This are packages located in Zuri's global packages directory (typically `$ZURI_INSTALL_DIR/vendor`) and are available to all applications in Zuri and can be imported in the same way as the core libraries, but not part of the core packages.
- **Local libraries**: This are packages located in your application, package or library's local packages directory (typically `$APPLICATION_DIR/.zuri/libs`) and are available to all modules in your application and can be imported in the same way as the core libraries.

Nyssa lets you install both global and local libraries for your application. The default behavior is to install a package locally.

### Installing a package

To install a package locally into your application, open a terminal at the root of your application and run

```
nyssa install <package_name>
```

This will create the path `.zuri/libs` in your current directory if it doesn't exist already and download the package into that directory. This will also update the `nyssa.json` file with the installed dependency uder the `deps` section.

> **NOTE:** nyssa does not install a local package outside of a nyssa project.

If you want to install a specific version of a package, you have you specify the version in the install command using the `@` separator like below.

```
nyssa install <package_name>@<package_version>
```

For example,

```
nyssa install packgist@1.1.3
```

To install a package globally and make it available to all applications, you need to add the `--global` flag (or `-g` for short) to the command. For example:

```
nyssa install --global <package_name>
```

> **NOTE:** You can install a package globally from any directory on your device. A `nyssa.json` file will only be updated if it is ran inside a nyssa project.

You can also choose to install a package from another repository other than [nyssa.zurilang.org](https://nyssa.zurilang.org) by specifying the `--repo` flag (or `-r` for short) like below.

```
nyssa install --repo <repo_url> <package_name>
```

The new repo will be added as a source into the `nyssa.json` script. This allows nyssa to easily restore the dependency later on.

> When installing packages, by default nyssa does not use the cache. You can instruct nyssa to check if a version of the package and version has been previously downloaded and use that download copy again by specifying the `--use-cache` flag (`-c` for short). This helps avoid unnecessary bandwidth costs.

### Uninstalling packages

To uninstall or remove a dependency from a project, you need to run the `nyssa uninstall` command like

```
nyssa uninstall <package_name>
```

To uninstall a package that was installed globally, you need to specify the `--global` flag.

```
nyssa uninstall --global <package_name>
```

### Upgrading packages

To upgrade Nyssa packages whether globally installed or locally installed, you only need to reinstall it. Nyssa will install the latest version of that package and replace the current one with it.

### Restoring packages

When nyssa creates a project, it includes a `.gitignore` file that tells it to ignore the `.zuri` directory. This keeps source control light and easy to manage. To restore back all your packages when you move or copy your code across locations or devices, you can run the command

```
nyssa restore
```

This is also useful if you ever mistakenly delete the `.zuri` directory or compress the project with archivers that ignore hidden file on Unix machines or somehow end up with a corrupted library. You can also use restore to reinstall all packages.

> Unlike package installation, restore uses a cache by default. Sometimes, this isn't exactly what you want. You can ask nyssa to not use any cached version by specifying the `--no-cache` flag (or `-x` for short).

