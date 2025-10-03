{ lib, stdenv, fetchFromGitHub, deno, makeWrapper, nodejs, npm }:

stdenv.mkDerivation rec {
  pname = "rusty-gun";
  version = "1.0.0";

  src = fetchFromGitHub {
    owner = "rusty-gun";
    repo = "rusty-gun";
    rev = "v${version}";
    sha256 = "PLACEHOLDER_SHA256";
  };

  nativeBuildInputs = [ deno makeWrapper nodejs npm ];

  buildPhase = ''
    # Build the web UI
    cd web/svelte
    npm install
    npm run build
    cd ../..

    # Compile the Deno application
    deno compile -A --output rusty-gun src/main.ts
  '';

  installPhase = ''
    # Install the binary
    mkdir -p $out/bin
    cp rusty-gun $out/bin/

    # Install the web UI
    mkdir -p $out/share/rusty-gun/web
    cp -r web/dist $out/share/rusty-gun/web/

    # Install configuration files
    mkdir -p $out/share/rusty-gun/config
    cp deno.json $out/share/rusty-gun/config/
    cp src/config.ts $out/share/rusty-gun/config/

    # Create a wrapper script that sets the web UI path
    makeWrapper $out/bin/rusty-gun $out/bin/rusty-gun-server \
      --set RUSTY_GUN_WEB_PATH "$out/share/rusty-gun/web/dist" \
      --set RUSTY_GUN_CONFIG_PATH "$out/share/rusty-gun/config"

    # Install man pages
    mkdir -p $out/share/man/man1
    cat > $out/share/man/man1/rusty-gun.1 << EOF
.TH RUSTY-GUN 1 "2024" "Rusty Gun ${version}" "User Commands"
.SH NAME
rusty-gun \- P2P Graph Database with SQLite Compatibility
.SH SYNOPSIS
.B rusty-gun
[\fIOPTIONS\fR] \fICOMMAND\fR
.SH DESCRIPTION
Rusty Gun is a local-first, peer-to-peer graph database with SQLite compatibility.
It provides offline-first data storage, encrypted data sharing, cross-device sync,
and a comprehensive web UI for data exploration and management.
.SH OPTIONS
.TP
\fB--port\fR, \fB-p\fR
Port to run the server on (default: 34567)
.TP
\fB--host\fR, \fB-h\fR
Host to bind to (default: localhost)
.TP
\fB--config\fR, \fB-c\fR
Path to configuration file
.TP
\fB--help\fR
Show help message
.SH COMMANDS
.TP
\fBserve\fR
Start the Rusty Gun server
.TP
\fBput\fR \fIKEY\fR \fIVALUE\fR
Store a key-value pair
.TP
\fBget\fR \fIKEY\fR
Retrieve a value by key
.TP
\fBdelete\fR \fIKEY\fR
Delete a key-value pair
.TP
\fBvsearch\fR \fIQUERY\fR
Perform vector search
.TP
\fBtype\fR \fITYPE\fR
List nodes of a specific type
.TP
\fBinstances\fR
List all node instances
.TP
\fBlist\fR
List all nodes
.TP
\fBconfig\fR \fISUBCOMMAND\fR
Manage configuration
.SH EXAMPLES
.TP
Start server:
.B rusty-gun serve --port 8080
.TP
Store data:
.B rusty-gun put "user:123" '{"name": "John", "email": "john@example.com"}'
.TP
Retrieve data:
.B rusty-gun get "user:123"
.TP
Vector search:
.B rusty-gun vsearch "machine learning"
.SH FILES
.TP
\fB~/.rusty-gun/\fR
Configuration directory
.TP
\fB~/.rusty-gun/data.sqlite\fR
Database file
.SH AUTHOR
Rusty Gun Team
.SH SEE ALSO
https://github.com/rusty-gun/rusty-gun
EOF
  '';

  meta = with lib; {
    description = "P2P Graph Database with SQLite Compatibility";
    homepage = "https://github.com/rusty-gun/rusty-gun";
    license = licenses.mit;
    maintainers = [ ];
    platforms = platforms.all;
    mainProgram = "rusty-gun";
  };
}
