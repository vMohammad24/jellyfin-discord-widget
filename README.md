# Jellyfin Discord Widget

 A discord widget featuring your Jellyfin media history and playback status. 

<p align="center">
  <img src="https://cdn.nest.rip/uploads/3f91c911-b5da-4bd0-bdb1-79c75b2abc61.png" alt="Discord Profile Widget Screenshot" />
</p>

## How it works

1. log in with Discord OAuth on the dashboard.
2. You connect your Jellyfin server (using credentials, a token, or Quick Connect).
3. Add the widget on discord.
4. enjoy


## Setup

1. Setup a PostgreSQL database.
2. Copy `.env.example` to `.env` and configure your credentials:

```bash
  cp .env.example .env
```
3. Run the application with `cargo run` or dockerize it using the provided `Dockerfile`.

## LICENSE
This project is licensed under the AGPLv3 License. See the [LICENSE](LICENSE) file for details.

## Contributing
Contributions are welcome! Please open an issue or submit a pull request with your changes.
