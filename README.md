# trexd

A tiny reverse proxy daemon.

```sh
curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSL https://github.com/parksb/trexd/releases/latest/download/install.sh | sudo sh
```

## configuration

```sh
$ mkdir -p /etc/trexd
$ cat > /etc/trexd/apps.json <<EOF
[
  {
    "hostname": "service.example.com",
    "addr": "127.0.0.1",
    "port": 8001,
    "tls": {
      "cert_path": "/path/to/cert/cert.pem",
      "key_path": "/path/to/key/privkey.pem"
    }
  },
  {
    "hostname": "api.example.com",
    "addr": "127.0.0.1",
    "port": 8002,
    "tls": {
      "cert_path": "/path/to/cert/cert.pem",
      "key_path": "/path/to/key/privkey.pem"
    }
  }
]
EOF
```

## systemd

```sh
$ cp systemd/config.conf /etc/trexd/config.conf
$ cp systemd/trexd.service /etc/systemd/system/trexd.service
$ systemctl daemon-reload
$ systemctl start trexd.service
$ systemctl enable trexd.service
```

## docker

```sh
$ docker pull ghcr.io/parksb/trexd:latest
$ docker run -d --name trexd -v /etc/trexd:/etc/trexd --network host --restart unless-stopped ghcr.io/parksb/trexd:latest
```
