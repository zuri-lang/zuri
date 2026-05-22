# Hosting a private repository

Nyssa is a self-hostable repository.

This means that you can install and configure your own private or custom instance of [nyssa.zurilang.org](https://nyssa.zurilang.org) that people can publish to and install packages from. In fact, [nyssa.zurilang.org](https://nyssa.zurilang.org) is essentially just another private repository that is accessible to everyone in the Zuri community and hosted on the public web.

### Starting the respository server

Once you have Nyssa installed on a device, you can start the repository server by running the command `nyssa serve`. This starts a local repository server that listens to the port `3000` by default. You should see a log similar to the following:

```
$ nyssa serve
Nyssa repository server started.
Repository URL: http://127.0.0.1:3000
```

You can visit `http://127.0.0.1:3000` in your browser to see your own clone of this website. 

Replace IP address with the address/domain of the server if you are trying to access it outside of the server. In fact, if you'll be accessing it ourside the local machine, it is advisable to change the configured host from `127.0.0.1` to `0.0.0.0`.

You can choose to run your server on another port or change the bind address using the `--port` (or `-p`) and `--host` (or `-n`) flag respectively.

For example:

```
$ nyssa serve --port 4000
Nyssa repository server started.
Repository URL: http://127.0.0.1:4000
```

The following example binds to `IP Any` (i.e. `0.0.0.0`) and listens on port `8000`.

```
$ nyssa serve --host 0.0.0.0 --port 8000
Nyssa repository server started.
Repository URL: http://localhost:8000
```

### Customizing your installation

You fully customize your own repository setup by modifying the `apps/nyssa/app/setup.zu` file in your Zuri installation. For example, you can change the host and port number from code by modifying the value of `REPOSITORY_HOST` and `REPOSITORY_PORT` respectively in the file `setup.zu`.

### Production considerations

- While the built-in server can be used as is on production, it is not advisable to do so unless you are not exposing it on the internet (such as internal use on in an organization). Especially if you are trying to run on `https`.
- Consider running behind a reverse-proxy such as `apache` or `lightspeed` or `nginx` or run it behind a `VPN`.

