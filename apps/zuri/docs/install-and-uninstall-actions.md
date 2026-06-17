# Install and Uninstall actions

Zuri provides two installation hook (`post_install` and `cli`) and one uninstallation hook (`pre_uninstall`) that allows package and library authors to customize the installation and uninstallation experience and do many things such as downloading and building extra dependencies that may be written in other programming languages.

### `post_install`

The `post_install` configuration allows package authors to specify a Zuri script that should be run after the package has been extracted into its destination directory. To specify a script to run after installation, add the `post_install` option to the `project.json` file.

```json
...
"post_install": "my_cli_script.zu"
```

The _<scirpt_name>_ must be a path or filename relative to the root of the package.

### `pre_uninstall`

The `pre_uninstall` configuration is much like the `post_install` configuration, except that it runs just before a package is uninstalled. You can add it to the `project.json` file in the same way as the `post_install` option.

```json
...
"pre_uninstall": "my_cli_script.zu"
```

### `cli`

The `cli` installation hook allows package authors to specify a script that serves as the CLI entry point to the application. When the _CLI_ script is specified, a CLI entry will be created at `.zuri` for local installations or at the root of Zuri for global installations. This files will be automatically removed during uninstallation.

For example, a testing framework `qi` specifies a CLI entry point. For this reason, when you install `qi` locally, you can run the Qi CLI app by simply running the command `qi` (or `.zuri\qi` for Windows) to run Qi. This is made possible because during installation, Zuri will automatically create the corresponding command-line entry file for you.

For applications installated globally, the application will become available on the user terminal **via the name** of the application provided that Zuri has been added to path during installation.

For example,

```json
"cli": "my_cli_script.zu"
```
