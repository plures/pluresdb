{ lib, stdenv, fetchFromGitHub, deno, makeWrapper, nodejs, npm }:

stdenv.mkDerivation rec {
  pname = "pluresdb";
  version = "1.0.0";

  src = fetchFromGitHub {
    owner = "pluresdb";
    repo = "pluresdb";
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
    deno compile -A --output pluresdb src/main.ts
  '';

  installPhase = ''
    # Install the binary
    mkdir -p $out/bin
    cp pluresdb $out/bin/

    # Install the web UI
    mkdir -p $out/share/pluresdb/web
    cp -r web/dist $out/share/pluresdb/web/

    # Install configuration files
    mkdir -p $out/share/pluresdb/config
    cp deno.json $out/share/pluresdb/config/
    cp src/config.ts $out/share/pluresdb/config/

    # Create a wrapper script that sets the web UI path
    makeWrapper $out/bin/pluresdb $out/bin/pluresdb-server \
      --set PLURESDB_WEB_PATH "$out/share/pluresdb/web/dist" \
      --set PLURESDB_CONFIG_PATH "$out/share/pluresdb/config"

    # Install man pages
    mkdir -p $out/share/man/man1
    cat > $out/share/man/man1/pluresdb.1 << EOF
.TH PLURESDB 1 "2024" "PluresDB ${version}" "User Commands"
.SH NAME
pluresdb \- P2P Graph Database with SQLite Compatibility
.SH SYNOPSIS
.B pluresdb
[\fIOPTIONS\fR] \fICOMMAND\fR
.SH DESCRIPTION
PluresDB is a local-first, peer-to-peer graph database with SQLite compatibility.
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
Start the PluresDB server
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
.B pluresdb serve --port 8080
.TP
Store data:
.B pluresdb put "user:123" '{"name": "John", "email": "john@example.com"}'
.TP
Retrieve data:
.B pluresdb get "user:123"
.TP
Vector search:
.B pluresdb vsearch "machine learning"
.SH FILES
.TP
\fB~/.pluresdb/\fR
Configuration directory
.TP
\fB~/.pluresdb/data.sqlite\fR
Database file
.SH AUTHOR
PluresDB Team
.SH SEE ALSO
https://github.com/pluresdb/pluresdb
EOF
  '';

  meta = with lib; {
    description = "P2P Graph Database with SQLite Compatibility";
    homepage = "https://github.com/pluresdb/pluresdb";
    license = licenses.mit;
    maintainers = [ ];
    platforms = platforms.all;
    mainProgram = "pluresdb";
  };
}
