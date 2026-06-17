# Hosting a private repository

Zuri is a self-hostable repository.

This means that you can install and configure your own private or custom instance of [zuri.zurilang.org](https://zuri.zurilang.org) that people can publish to and install packages from. In fact, [zuri.zurilang.org](https://zuri.zurilang.org) is essentially just another private repository that is accessible to everyone in the Zuri community and hosted on the public web.

### Starting the respository server

Once you have Zuri installed on a device, you can start the repository server by running the command `zuri serve`. This starts a local repository server that listens to the port `3000` by default. You should see a log similar to the following:

```
$ zuri serve
Zuri repository server started.
Repository URL: http://127.0.0.1:3000
```

You can visit `http://127.0.0.1:3000` in your browser to see your own clone of this website. 

Replace IP address with the address/domain of the server if you are trying to access it outside of the server. In fact, if you'll be accessing it ourside the local machine, it is advisable to change the configured host from `127.0.0.1` to `0.0.0.0`.

You can choose to run your server on another port or change the bind address using the `--port` (or `-p`) and `--host` (or `-n`) flag respectively.

For example:

```
$ zuri serve --port 4000
Zuri repository server started.
Repository URL: http://127.0.0.1:4000
```

The following example binds to `IP Any` (i.e. `0.0.0.0`) and listens on port `8000`.

```
$ zuri serve --host 0.0.0.0 --port 8000
Zuri repository server started.
Repository URL: http://localhost:8000
```

### Customizing your installation

You can customize your own repository setup by setting a few environment variables:

- `ZURI_REPOSITORY_HOST`: The host address on which Zuric runs (default is `0.0.0.0`)
- `ZURI_REPOSITORY_PORT`: The port number on which Zuric runs (default is `3000`)
- `ZURI_STORAGE_DIR`: Where to store Zuric files such as database, cache, package archives, logs, etc.
- `ZURI_SESSION_NAME`: The name of the Zuric session cookie.

While not advised, you can also fully customize your own repository setup by modifying the `apps/zuri/app/setup.zu` file in your Zuri installation. For example, you can change the host and port number from code by modifying the value of `REPOSITORY_HOST` and `REPOSITORY_PORT` respectively in the file `setup.zu`. While this options allows for nearly any form of customization, it is also very easy to break your Zuri installation by modifying the files directly!

### Production considerations

- While the built-in server can be used as is on production, it is not advisable to do so unless you are not exposing it on the internet (such as internal use on in an organization). Especially if you are trying to run on `https`.
- Consider running behind a reverse-proxy such as `apache` or `lightspeed` or `nginx` or run it behind a `VPN`.

