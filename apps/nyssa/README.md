[![License](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/zuri-lang/zuri/blob/master/LICENSE)
[![Coverage Status](https://coveralls.io/repos/github/zuri-lang/nyssa/badge.svg?branch=main)](https://coveralls.io/github/zuri-lang/zuri?branch=main)
[![Version](https://img.shields.io/badge/version-0.1.4-green)](https://github.com/zuri-lang/zuri)

# Nyssa

Nyssa is the official package manager for the Zuri programming language. It is also a self-hostable repository server that allows you to easily manage and distribute packages for your Zuri projects.

#### The CLI

![Nyssa CLI](https://raw.githubusercontent.com/zuri-lang/zuri/main/apps/nyssa/nyssa-cli.png)

#### The browsable repository website.

![Nyssa Repository](https://raw.githubusercontent.com/zuri-lang/zuri/main/apps/nyssa/nyssa.png)


## Features

- [x] Create packages.
- [x] Manage application dependencies. 
  - [x] Install package.
  - [x] Uninstall package.
  - [x] Update (Install without specifying a version).
  - [x] Restore package.
  - [x] Publish package.
- [x] Built-in hostable repository server.
- [x] Publish package to public and private repositories.
- [x] Nyssa repository server public API.
- [x] Nyssa repository server searchable frontend website.
- [x] Manage publisher accounts.
  - [x] Create a publisher account.
  - [x] Login to a publisher account.
  - [x] Logout from the publisher account.
  - [x] Account Recovery.
- [x] Custom Post-Installation script support.
- [x] Custom Pre-Uninstallation script support.
- [ ] Fetch packages from CVS sources.
- [ ] Generate application/library documentation.
- [x] Test Runner.
- [ ] C Extension compiler.


## Documentation

To read the documentation, see the [Getting started](https://nyssa.zurilang.org/docs) guide on the website.


## Contributing

We welcome contributions from the community. Please see the [CONTRIBUTING.md](CONTRIBUTING.md) file for more information on how you can help.


## License

Nyssa is released under the [MIT license](LICENSE).
