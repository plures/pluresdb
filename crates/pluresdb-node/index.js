const { existsSync, readFileSync } = require("fs");
const { join } = require("path");

const { platform, arch } = process;

let nativeBinding = null;
let localFileExisted = false;
let loadError = null;

function isMusl() {
  // For non-glibc Linux systems we must check if musl is used instead
  if (!process.report || typeof process.report.getReport !== "function") {
    try {
      const lddPath = require("child_process")
        .execSync("which ldd")
        .toString()
        .trim();
      return readFileSync(lddPath, "utf8").includes("musl");
    } catch (e) {
      return true;
    }
  } else {
    const { glibcVersionRuntime } = process.report.getReport().header;
    return !glibcVersionRuntime;
  }
}

switch (platform) {
  case "android":
    switch (arch) {
      case "arm64":
        localFileExisted = existsSync(
          join(__dirname, "pluresdb-node.android-arm64.node")
        );
        try {
          if (localFileExisted) {
            nativeBinding = require("./pluresdb-node.android-arm64.node");
          } else {
            nativeBinding = require("@plures/pluresdb-native-android-arm64");
          }
        } catch (e) {
          loadError = e;
        }
        break;
      case "arm":
        localFileExisted = existsSync(
          join(__dirname, "pluresdb-node.android-arm-eabi.node")
        );
        try {
          if (localFileExisted) {
            nativeBinding = require("./pluresdb-node.android-arm-eabi.node");
          } else {
            nativeBinding = require("@plures/pluresdb-native-android-arm-eabi");
          }
        } catch (e) {
          loadError = e;
        }
        break;
      default:
        throw new Error(`Unsupported architecture on Android ${arch}`);
    }
    break;
  case "win32":
    switch (arch) {
      case "x64":
        localFileExisted = existsSync(
          join(__dirname, "pluresdb-node.win32-x64-msvc.node")
        );
        try {
          if (localFileExisted) {
            nativeBinding = require("./pluresdb-node.win32-x64-msvc.node");
          } else {
            nativeBinding = require("@plures/pluresdb-native-win32-x64-msvc");
          }
        } catch (e) {
          loadError = e;
        }
        break;
      case "ia32":
        localFileExisted = existsSync(
          join(__dirname, "pluresdb-node.win32-ia32-msvc.node")
        );
        try {
          if (localFileExisted) {
            nativeBinding = require("./pluresdb-node.win32-ia32-msvc.node");
          } else {
            nativeBinding = require("@plures/pluresdb-native-win32-ia32-msvc");
          }
        } catch (e) {
          loadError = e;
        }
        break;
      case "arm64":
        localFileExisted = existsSync(
          join(__dirname, "pluresdb-node.win32-arm64-msvc.node")
        );
        try {
          if (localFileExisted) {
            nativeBinding = require("./pluresdb-node.win32-arm64-msvc.node");
          } else {
            nativeBinding = require("@plures/pluresdb-native-win32-arm64-msvc");
          }
        } catch (e) {
          loadError = e;
        }
        break;
      default:
        throw new Error(`Unsupported architecture on Windows: ${arch}`);
    }
    break;
  case "darwin":
    localFileExisted = existsSync(
      join(__dirname, "pluresdb-node.darwin-universal.node")
    );
    try {
      if (localFileExisted) {
        nativeBinding = require("./pluresdb-node.darwin-universal.node");
      } else {
        nativeBinding = require("@plures/pluresdb-native-darwin-universal");
      }
      break;
    } catch {}
    switch (arch) {
      case "x64":
        localFileExisted = existsSync(
          join(__dirname, "pluresdb-node.darwin-x64.node")
        );
        try {
          if (localFileExisted) {
            nativeBinding = require("./pluresdb-node.darwin-x64.node");
          } else {
            nativeBinding = require("@plures/pluresdb-native-darwin-x64");
          }
        } catch (e) {
          loadError = e;
        }
        break;
      case "arm64":
        localFileExisted = existsSync(
          join(__dirname, "pluresdb-node.darwin-arm64.node")
        );
        try {
          if (localFileExisted) {
            nativeBinding = require("./pluresdb-node.darwin-arm64.node");
          } else {
            nativeBinding = require("@plures/pluresdb-native-darwin-arm64");
          }
        } catch (e) {
          loadError = e;
        }
        break;
      default:
        throw new Error(`Unsupported architecture on macOS: ${arch}`);
    }
    break;
  case "freebsd":
    if (arch !== "x64") {
      throw new Error(`Unsupported architecture on FreeBSD: ${arch}`);
    }
    localFileExisted = existsSync(
      join(__dirname, "pluresdb-node.freebsd-x64.node")
    );
    try {
      if (localFileExisted) {
        nativeBinding = require("./pluresdb-node.freebsd-x64.node");
      } else {
        nativeBinding = require("@plures/pluresdb-native-freebsd-x64");
      }
    } catch (e) {
      loadError = e;
    }
    break;
  case "linux":
    switch (arch) {
      case "x64":
        if (isMusl()) {
          localFileExisted = existsSync(
            join(__dirname, "pluresdb-node.linux-x64-musl.node")
          );
          try {
            if (localFileExisted) {
              nativeBinding = require("./pluresdb-node.linux-x64-musl.node");
            } else {
              nativeBinding = require("@plures/pluresdb-native-linux-x64-musl");
            }
          } catch (e) {
            loadError = e;
          }
        } else {
          localFileExisted = existsSync(
            join(__dirname, "pluresdb-node.linux-x64-gnu.node")
          );
          try {
            if (localFileExisted) {
              nativeBinding = require("./pluresdb-node.linux-x64-gnu.node");
            } else {
              nativeBinding = require("@plures/pluresdb-native-linux-x64-gnu");
            }
          } catch (e) {
            loadError = e;
          }
        }
        break;
      case "arm64":
        if (isMusl()) {
          localFileExisted = existsSync(
            join(__dirname, "pluresdb-node.linux-arm64-musl.node")
          );
          try {
            if (localFileExisted) {
              nativeBinding = require("./pluresdb-node.linux-arm64-musl.node");
            } else {
              nativeBinding = require("@plures/pluresdb-native-linux-arm64-musl");
            }
          } catch (e) {
            loadError = e;
          }
        } else {
          localFileExisted = existsSync(
            join(__dirname, "pluresdb-node.linux-arm64-gnu.node")
          );
          try {
            if (localFileExisted) {
              nativeBinding = require("./pluresdb-node.linux-arm64-gnu.node");
            } else {
              nativeBinding = require("@plures/pluresdb-native-linux-arm64-gnu");
            }
          } catch (e) {
            loadError = e;
          }
        }
        break;
      case "arm":
        localFileExisted = existsSync(
          join(__dirname, "pluresdb-node.linux-arm-gnueabihf.node")
        );
        try {
          if (localFileExisted) {
            nativeBinding = require("./pluresdb-node.linux-arm-gnueabihf.node");
          } else {
            nativeBinding = require("@plures/pluresdb-native-linux-arm-gnueabihf");
          }
        } catch (e) {
          loadError = e;
        }
        break;
      default:
        throw new Error(`Unsupported architecture on Linux: ${arch}`);
    }
    break;
  default:
    throw new Error(`Unsupported OS: ${platform}, architecture: ${arch}`);
}

if (!nativeBinding) {
  if (loadError) {
    throw loadError;
  }
  throw new Error(`Failed to load native binding`);
}

const { PluresDatabase, init } = nativeBinding;

module.exports.PluresDatabase = PluresDatabase;
module.exports.init = init;
