# Publishers account

Nyssa allows you to publish packages to the default repository ([nyssa.zurilang.org](https://nyssa.zurilang.org)) as well as private hosted repositories through the use of publisher accounts.

Publisher accounts are account created by package authors to allow them publish nyssa package to repositories. A nyssa instance can only publish to one repository at a time and as such, you can only be authenticated to one Nyssa repository at a time.

### Creating publisher accounts

Publisher accounts can only be created through the nyssa CLI and never from the repository web application. 

To create a new publisher account on the default repository [nyssa.zurilang.org](https://nyssa.zurilang.org), run the command `nyssa account create`. This will bring up a new prompt asking you for your `username`, `email` and `password` all of which are required to create a new account.

Upon successful account creation, you'll be given your issued **Publisher's Key**. This key in conjuction with your username allows you to publish packages.

> Save your publisher's key in a secure and safe location because you'll need it to recover your account should you ever forget your account password.

To create an account in another repository or a private repository, use the `--repo` flag to point to another repository. For example

```
nyssa account --repo https://myprivatenyssa.com create
```

> You can use the short form `-r` instead of the long `--repo`.

### Login to publisher account

If you've created a publisher account, you can always login on the same or other devices using the command `nyssa account login`. This will prompt you for your `username` and `password`. Upon successful login, your key will be registered against the current machine.

Just like creating a new account, you can use the `-r` or `--repo` to login to another repository different from the [nyssa.zurilang.org](https://nyssa.zurilang.org).

> You can also login to your publisher account on your repository's website [login](/login) page.


### Loging out of an account

When you try to sign into another account when you are already signed in to an account, nyssa asks if you want to logout of the former account. simply enter `y` and nyssa automatically logs out the former account signed in.

To explicitly sign out of all accounts, run the command `nyssa account logout`.

