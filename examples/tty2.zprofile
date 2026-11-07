# Append to ~/.zprofile after installing uim-fcitx5.
# uim-fep exports UIM_FEP_PID before starting the child login shell.
if [[ -o interactive && $TERM == kmscon && ${XDG_VTNR:-} == 2 && -z ${UIM_FEP_PID:-} ]] \
    && [[ -x /usr/local/bin/uim-fcitx5 ]]; then
    if /usr/local/bin/uim-fcitx5; then
        exit
    fi
    print -u2 'uim-fcitx5 failed; continuing with the normal shell.'
fi
