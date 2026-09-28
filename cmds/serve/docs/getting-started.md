# Getting started

{{site}} holds packages for Zuri. Anything published here can be
added to a project with one command, and anything you write can be
published here with another.

You need an account to publish. You need nothing at all to install.

## Installing a package

From inside a project, the directory with a `project.toml`:

```sh
zuri install http-extra
```

That finds the newest version, installs it into `.zuri/libs`, records
it in `project.toml` and pins the exact version in `project.lock`. Import
it by its name, with every hyphen made an underscore:

```zuri
import http_extra
```

A package that is not on the default registry is named by the
registry it comes from:

```sh
zuri install http-extra --registry {{url}}
```

## Finding one

Search from the box at the top of every page, or from the command
line:

```sh
zuri search json
zuri info http-extra
```

## Where to go next

- [Installing packages](/docs/installing) covers versions, the
  lockfile, and installing for your user rather than one project.
- [Publishing packages](/docs/publishing) covers what a package needs
  and how a version is published.
- [Accounts and tokens](/docs/accounts) covers signing the command line
  in.
- [Hosting a repository](/docs/hosting) covers running one of these
  yourself.
