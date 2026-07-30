#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage:" >&2
  echo "  prism-uri-handler.sh render APPIMAGE DATA_ROOT OUTPUT.desktop" >&2
  echo "  prism-uri-handler.sh register APPIMAGE DATA_ROOT" >&2
  exit 2
}

validate_path() {
  local path="$1"
  local unsupported='["$`\\]'
  if [[ "$path" =~ $unsupported || "$path" == *$'\n'* || "$path" == *$'\r'* ]]; then
    echo "unsupported character in path: $path" >&2
    exit 1
  fi
}

render_handler() {
  local appimage="$1"
  local data_root="$2"
  local output="$3"

  [[ -x "$appimage" ]] || {
    echo "Prism AppImage is not executable: $appimage" >&2
    exit 1
  }
  [[ -d "$data_root" ]] || {
    echo "isolated Prism data root does not exist: $data_root" >&2
    exit 1
  }
  [[ ! -e "$output" ]] || {
    echo "refusing to overwrite desktop entry: $output" >&2
    exit 1
  }
  command -v unsquashfs >/dev/null || {
    echo "unsquashfs is required to verify Prism's URI scheme" >&2
    exit 1
  }

  appimage="$(realpath "$appimage")"
  data_root="$(realpath "$data_root")"
  validate_path "$appimage"
  validate_path "$data_root"

  local offset
  offset="$("$appimage" --appimage-offset)"
  local embedded_entry
  embedded_entry="$(
    unsquashfs -o "$offset" -cat "$appimage" \
      org.prismlauncher.PrismLauncher.desktop
  )"
  if ! grep -Fq 'x-scheme-handler/prismlauncher' <<<"$embedded_entry"; then
    echo "Prism AppImage does not advertise the prismlauncher URI scheme" >&2
    exit 1
  fi

  mkdir -p "$(dirname "$output")"
  {
    printf '%s\n' '[Desktop Entry]'
    printf '%s\n' 'Type=Application'
    printf '%s\n' 'Name=Ravel isolated Prism URI handler'
    printf 'Exec="%s" -d "%s" %%U\n' "$appimage" "$data_root"
    printf '%s\n' 'Terminal=false'
    printf '%s\n' 'NoDisplay=true'
    printf '%s\n' 'MimeType=x-scheme-handler/prismlauncher;'
  } >"$output"
  chmod 0644 "$output"
}

[[ "$#" -ge 1 ]] || usage
mode="$1"
shift
case "$mode" in
render)
  [[ "$#" -eq 3 ]] || usage
  render_handler "$1" "$2" "$3"
  ;;
register)
  [[ "$#" -eq 2 ]] || usage
  command -v xdg-mime >/dev/null || {
    echo "xdg-mime is required to register the URI handler" >&2
    exit 1
  }
  application_root="${XDG_DATA_HOME:-"$HOME/.local/share"}/applications"
  handler="$application_root/ravel-isolated-prism.desktop"
  render_handler "$1" "$2" "$handler"
  xdg-mime default ravel-isolated-prism.desktop x-scheme-handler/prismlauncher
  printf 'registered %s\n' "$handler"
  ;;
*)
  usage
  ;;
esac
