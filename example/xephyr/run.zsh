#!/usr/bin/env zsh

xinit ${0:A:h}/xinitrc -- \
    =Xephyr \
        :100 \
        -ac \
        -screen 800x600 \
        -host-cursor
