# 🧪 Test Lab Deployment (Samba AD)

This directory contains a `docker-compose.yml` to deploy an Active Directory Compatible Domain Controller (DC) using Samba 4.

## 📋 Requirements

*   Docker
*   Docker Compose
*   Linux (Ubuntu Server / Debian recommended)

## ⚠️ Conflict with `systemd-resolved` (Ubuntu/Debian)

Samba AD needs to act as a DNS server and listen on port **53**. On modern distributions like Ubuntu, this port is usually occupied by `systemd-resolved`.

If you try to start the container with `network_mode: host` without fixing this, you will get an error stating that port 53 is in use.

### Solution

We must disable the "Stub Listener" of `systemd-resolved` to free up port 53 on the local interface.

1.  Edit the configuration file:
    ```bash
    sudo nano /etc/systemd/resolved.conf
    ```

2.  Uncomment and change the `DNSStubListener` line:
    ```ini
    [Resolve]
    # ...
    DNSStubListener=no
    ```

3.  Create a symbolic link so `/etc/resolv.conf` points to the correct configuration (since systemd-resolved will stop managing the stub on 127.0.0.53):
    ```bash
    sudo ln -sf /run/systemd/resolve/resolv.conf /etc/resolv.conf
    ```

4.  Restart the service:
    ```bash
    sudo systemctl restart systemd-resolved
    ```

5.  Verify that port 53 is free:
    ```bash
    sudo lsof -i :53
    # Output should be empty or only show processes other than systemd-resolve listening on 127.0.0.53
    ```

## 🚀 Deployment

1.  Edit the `docker-compose.yml` file and ensure you change the `HOSTIP` variable to your host machine's static IP.
    ```yaml
    environment:
      - HOSTIP=192.168.1.50 # <--- Your IP here
    ```

2.  Start the service:
    ```bash
    docker compose up -d
    ```

3.  Check the logs to confirm Samba has started correctly (it may take a moment the first time while provisioning the domain):
    ```bash
    docker compose logs -f
    ```

## 🔍 Verification

Once up, you can test DNS resolution and LDAP connection from the same machine or another on the network.

```bash
# Test DNS (Samba should respond)
nslookup dc-lab.lab.local 192.168.1.50

# Test LDAP connection with mad
mad check
```

## 🧹 Cleanup

To stop and delete data (warning, the whole domain will be lost!):

```bash
docker compose down -v
```
