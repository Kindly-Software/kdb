#!/bin/bash
# Placeholder PNG asset generator
# Creates 1x1 pixel placeholders with correct filenames for size reference

convert -size 50x50 xc:"#9B59B6" StoreLogo.png
convert -size 44x44 xc:"#9B59B6" Square44x44Logo.png
convert -size 150x150 xc:"#9B59B6" Square150x150Logo.png
convert -size 310x150 xc:"#9B59B6" Wide310x150Logo.png
convert -size 310x310 xc:"#9B59B6" LargeTile.png

echo "Placeholder assets created. Replace with branded designs before submission."
