# Getting started

{{site}} is a registry of Zuri packages. You can add any package
published here to a project with a single command, and publish your own
packages here with another.

You need an account to publish packages, but not to install them.

## Installing a package

Run this from inside a project, which is a directory that contains a
`project.toml` file:

```sh
zuri install http-extra
```

This finds the newest version of the package, installs it in
`.zuri/libs`, adds it to `project.toml`, and records the exact version in
`project.lock`. To use the package, import it by its name, with each
hyphen replaced by an underscore:

```zuri
import http_extra
```

If this isn't your default registry, name it when you install a package
from it:

```sh
zuri install http-extra --registry {{url}}
```

## Finding a package

Use the search box at the top of every page, or search from the command
line. On the site, a search for a single word also looks through the
names that every package documents. For example, `forecast` or
`app.forecast` finds that function and its package, and links straight
to its documentation.

```sh
zuri search json
zuri info http-extra
```

## Next steps

- [Installing packages](/docs/installing) explains versions, the
  lockfile, and installing packages for your user instead of a single
  project.
- [Publishing packages](/docs/publishing) explains what a package needs,
  and how to publish a version.
- [Accounts and tokens](/docs/accounts) explains how to sign in from the
  command line.
- [Hosting a registry](/docs/hosting) explains how to run a registry
  like this one yourself.
