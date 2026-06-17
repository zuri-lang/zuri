# Commands

Usage: `nyssa` [ [-h] | [-v] ] [COMMAND]

### OPTIONS
-  `-h`, `--help`               Show this help message and exit
-  `-v`, `--version`            Show Nyssa version

### COMMANDS
- **account** <_choice_>        Manages a Nyssa publisher account
  -  `create`                       creates a new publisher account
  -  `login`                        login to a publisher account
  -  `logout`                       log out of a publisher account
  -  *`-r`*, *`--repo`* <_value_>   the repo where the account is located
- **clean**                     Clear Nyssa storage
  -  *`-c`*, *`--cache`*            clean packages cache
  -  *`-l`*, *`--logs`*             clean logs
  -  *`-a`*, *`--all`*              clean everything
- **info**                      Shows current project information
- **init**                      Creates a new package in current directory
  -  *`-n`*, *`--name`* <_value_>   the name of the package
- **install** <_value_>           Installs a Zuri package
  -  *`-g`*, *`--global`*           installs the package globally
  -  *`-c`*, *`--use-cache`*        enables the cache
  -  *`-r`*, *`--repo`* <*value*>   the repository to install from
- **publish**                   Publishes a repository
  -  *`-r`*, *`--repo`* <*value*>   repository url
- **restore**                   Restores all project dependencies
  -  *`-x`*, *`--no-cache`*         disables the cache
- **serve**                     Starts a local Nyssa repository server
  -  *`-p`*, *`--port`* <*value*>   port of the server (default: 3000)
  -  *`-n`*, *`--host`* <*value*>   the host ip (default: 127.0.0.1)
- **test**                      Run the tests
- **uninstall** <*value*>       Uninstalls a Zuri package
  -  *`-g`*, *`--global`*           package is a global package