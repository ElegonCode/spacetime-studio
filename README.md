![Spacetime Studio banner](https://raw.githubusercontent.com/ElegonCode/spacetime-studio/main/public/screenshot.png)

# Spacetime Studio

Spacetime Studio is a desktop admin client for [SpacetimeDB](https://spacetimedb.com/). It gives you a lightweight way to connect to a local or hosted SpacetimeDB instance, inspect schemas, browse and edit table data, run raw SQL, view reducers/functions, and tail database logs.

Windows builds are available from the GitHub Releases page. macOS and Linux builds are planned for a future release; for now, those platforms can build and run the app from source.

## What You Can Do

- Save connection profiles for local or hosted SpacetimeDB databases.
- Store bearer tokens in the operating system keychain instead of the frontend.
- See which saved servers are online and switch between them from the sidebar.
- Inspect tables, columns, access modes, and primary keys from the database schema.
- Browse table rows with pagination.
- Create, update, and delete rows through the table UI.
- Run raw SQL against the selected database.
- Browse reducers, lifecycle functions, and function parameters.
- Read database logs with live refresh.

## Tech Stack

- [Tauri 2](https://tauri.app/) for the desktop shell and native Rust backend.
- [Vue 3](https://vuejs.org/) and TypeScript for the frontend.
- [Vite](https://vite.dev/) for development and frontend builds.
- [Nuxt UI](https://ui.nuxt.com/) and Tailwind CSS for the interface.
- Rust `reqwest` for SpacetimeDB HTTP API calls.
- Rust `keyring` for secure token storage.

## Getting Started

There are two intended ways to use Spacetime Studio:

- **Download a release** for the easiest Windows setup.
- **Build from source** if you are on macOS or Linux, want the latest development version, or want to contribute.

### Option 1: Download A Release

Windows installers are available from the repository's GitHub Releases page.

1. Open the repository's GitHub Releases page.
2. Download the Windows installer from the latest release.
3. Install Spacetime Studio.
4. Launch the app and connect to your SpacetimeDB database.

This path does not require Node.js, Rust, or compiling the project yourself. macOS and Linux release downloads will be added in a future release.

### Option 2: Build From Source

Use this path if you want to run Spacetime Studio on macOS or Linux, use the latest development version, or contribute to the project.

#### Prerequisites

Install these before running the app:

- [Node.js](https://nodejs.org/) and npm.
- [Rust](https://www.rust-lang.org/tools/install).
- The platform dependencies required by Tauri 2 for your operating system.
- A running SpacetimeDB host to connect to.

For a local SpacetimeDB server, the default connection URL in the app is:

```txt
http://localhost:3000
```

You will also need the database name or database identity for the module you want to inspect.

Clone the repository:

```bash
git clone <repository-url>
cd spacetime-studio
```

Install JavaScript dependencies:

```bash
npm install
```

Run the desktop app in development mode:

```bash
npm run tauri dev
```

Tauri will start the Vite dev server and open the Spacetime Studio desktop window.

## Connecting To A Database

1. Open Spacetime Studio.
2. Click **Add connection**, either on the Tables page or in the connection picker at
   the top of the sidebar.
3. Enter a friendly profile name.
4. Pick a host:
   - **Local server** uses `http://localhost:3000`.
   - **Hosted in Maincloud** uses `https://maincloud.spacetimedb.com`.
   - **Self-hosted** lets you enter the public URL of a server you run yourself, for
     example a Railway, Fly, or VPS deployment. A bare domain such as
     `my-app.up.railway.app` is fine; `https://` is assumed when you leave the scheme
     off, except for loopback hosts, which assume `http://`.
5. Enter the database name or database identity.
6. Optionally enter an auth token for private databases or admin-only actions.
   SpacetimeDB uses a signed JWT sent as a bearer token on every host, self-hosted
   included. Tokens are signed by the issuing server, so a Maincloud token will not
   work against your own server and vice versa. To get a token from a server you host,
   add it with `spacetime server add`, run
   `spacetime login --server-issued-login <server>`, then copy the value from
   `spacetime login show --token`.
7. Click **Test** to verify the host and database can be reached.
8. Click **Save**. A new profile becomes the active connection straight away.

To switch connections, open the picker at the top of the sidebar. It re-checks every
saved server when it opens and labels each one **Online**, **Offline** (the host could
not be reached), or **Error** (the host answered but rejected the database or token).
Hover a connection to edit or delete it.

When editing an existing profile, leave the token field blank to keep the token that is
already in your OS keychain.

Connection profile metadata is stored locally by the app. Bearer tokens are stored in your OS keychain using the service name `spacetime-studio`.

## Using The App

Use the sidebar to move between the available workspace views:

- **Tables** is the home page. It shows the schema, table rows, row editing controls,
  and raw SQL runner. Tables are grouped into Private and Public sections.
- **Functions** lists reducers, lifecycle functions, and their parameters.
- **Logs** shows recent database logs, color-coded by level, and can refresh
  automatically.

Functions and Logs are disabled until the active connection is confirmed reachable.
If it can't be reached, the Tables page shows the error with options to retry or edit
the connection.

Some operations require database permissions. If a request fails with an authorization error, update the connection profile with a token that has the needed access.

## Build Commands

Type-check and build the frontend:

```bash
npm run build
```

Preview the built frontend in a browser:

```bash
npm run preview
```

Build a desktop bundle with Tauri:

```bash
npm run tauri build
```

Build outputs are generated by Tauri under `src-tauri/target/`.

## Project Structure

```txt
spacetime-studio/
+-- src/                 Vue frontend
|   +-- components/       Shared UI components
|   +-- lib/              Frontend API wrappers and helpers
|   +-- pages/            App views
+-- src-tauri/           Tauri and Rust backend
|   +-- src/spacetime.rs  SpacetimeDB profile, schema, SQL, row, and log commands
+-- public/              Static assets
+-- package.json         npm scripts and frontend dependencies
+-- README.md
```

## Development Notes

- The frontend calls Rust commands through Tauri `invoke`.
- SpacetimeDB HTTP requests happen in the Rust backend, not directly in the browser layer.
- Saved profiles live in the app data directory.
- Tokens are never returned to the frontend after they are saved.
- The app is early-stage, so expect rough edges and incomplete workflows.

## Contributing

Issues, ideas, and pull requests are welcome. Useful areas to help with include:

- Better onboarding and connection diagnostics.
- Safer row editing flows.
- Reducer invocation support.
- Frontend improvements, including layout, navigation, accessibility, and interaction polish.
- Tests around SpacetimeDB response parsing and SQL mutation helpers.
- Documentation for common local SpacetimeDB workflows.

When contributing, try to keep changes focused and include enough context in the pull request for someone else to test the behavior.

## License

Spacetime Studio is released under the [MIT License](LICENSE).
