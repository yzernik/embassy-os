#!/bin/bash

set -e

function partition_for () {
        if [[ "$1" =~ [0-9]+$ ]]; then
                echo "$1p$2"
        else
                echo "$1$2"
        fi
}

# Write contents of LOOPDEV (Ubuntu image) to sd card and make filesystems, then detach the loop device
echo USING $LOOPDEV TO IMAGE $OUTPUT_DEVICE
sudo dd if=${LOOPDEV}p1 of=`partition_for ${OUTPUT_DEVICE} 1` bs=1M iflag=fullblock oflag=direct conv=fsync status=progress
sudo mkfs.vfat -F 32 `partition_for ${OUTPUT_DEVICE} 2`
sudo dd if=${LOOPDEV}p2 of=`partition_for ${OUTPUT_DEVICE} 3` bs=1M iflag=fullblock oflag=direct conv=fsync status=progress
sudo mkfs.ext4 `partition_for ${OUTPUT_DEVICE} 4`

sudo losetup -d $LOOPDEV

# Label the filesystems
sudo fatlabel `partition_for ${OUTPUT_DEVICE} 1` system-boot
sudo fatlabel `partition_for ${OUTPUT_DEVICE} 2` EMBASSY
sudo e2label `partition_for ${OUTPUT_DEVICE} 3` green
sudo e2label `partition_for ${OUTPUT_DEVICE} 4` blue

# Mount the boot partition and config
mkdir -p /tmp/eos-mnt
sudo mount `partition_for ${OUTPUT_DEVICE} 1` /tmp/eos-mnt

if [[ "$ENVIRONMENT" =~ (^|-)dev($|-) ]]; then
	sudo cp build/user-data-dev /tmp/eos-mnt/user-data
else
	sudo cp build/user-data /tmp/eos-mnt/user-data
fi

sudo sed -i 's/PARTUUID=cb15ae4d-02/PARTUUID=cb15ae4d-03/g' /tmp/eos-mnt/cmdline.txt
sudo sed -i 's/ init=\/usr\/lib\/raspi-config\/init_resize.sh//g' /tmp/eos-mnt/cmdline.txt

cat /tmp/eos-mnt/config.txt | grep -v "dtoverlay=" | sudo tee /tmp/eos-mnt/config.txt.tmp
echo "dtoverlay=pwm-2chan,disable-bt" | sudo tee -a /tmp/eos-mnt/config.txt.tmp
sudo mv /tmp/eos-mnt/config.txt.tmp /tmp/eos-mnt/config.txt
sudo touch /tmp/eos-mnt/ssh

# Unmount the boot partition and mount embassy partition
sudo umount /tmp/eos-mnt
sudo mount `partition_for ${OUTPUT_DEVICE} 2` /tmp/eos-mnt
if [ "$NO_KEY" != "1" ]; then sudo cp product_key.txt /tmp/eos-mnt; else echo "This image is being written with no product key"; fi
sudo umount /tmp/eos-mnt

sudo mount `partition_for ${OUTPUT_DEVICE} 3` /tmp/eos-mnt

sudo mkdir  /tmp/eos-mnt/media/boot-rw
sudo mkdir  /tmp/eos-mnt/embassy-os
sudo cp build/fstab /tmp/eos-mnt/etc/fstab
sudo cp build/journald.conf /tmp/eos-mnt/etc/systemd/journald.conf
# Enter the backend directory, copy over the built EmbassyOS binaries and systemd services, edit the nginx config, then create the .ssh directory
cd backend/

sudo cp target/aarch64-unknown-linux-gnu/release/embassy-init /tmp/eos-mnt/usr/local/bin
sudo cp target/aarch64-unknown-linux-gnu/release/embassyd /tmp/eos-mnt/usr/local/bin
sudo cp target/aarch64-unknown-linux-gnu/release/embassy-cli /tmp/eos-mnt/usr/local/bin
sudo cp *.service /tmp/eos-mnt/etc/systemd/system/

cd ..

# Copy system images
sudo mkdir -p /tmp/eos-mnt/var/lib/embassy/system-images
sudo cp system-images/**/*.tar /tmp/eos-mnt/var/lib/embassy/system-images

# after performing npm run build
sudo mkdir -p /tmp/eos-mnt/var/www/html
sudo cp -R frontend/dist/diagnostic-ui /tmp/eos-mnt/var/www/html/diagnostic
sudo cp -R frontend/dist/setup-wizard /tmp/eos-mnt/var/www/html/setup
sudo cp -R frontend/dist/ui /tmp/eos-mnt/var/www/html/main
sudo cp index.html /tmp/eos-mnt/var/www/html/index.html

# Make the .ssh directory
sudo mkdir -p /tmp/eos-mnt/root/.ssh

# Custom MOTD
sudo rm /tmp/eos-mnt/etc/motd
sudo cp ./build/00-embassy /tmp/eos-mnt/etc/update-motd.d
sudo chmod -x /tmp/eos-mnt/etc/update-motd.d/*
sudo chmod +x /tmp/eos-mnt/etc/update-motd.d/00-embassy

if [[ "$ENVIRONMENT" =~ (^|-)dev($|-) ]]; then
	cat ./build/initialization.sh | grep -v "passwd -l start9" | sudo tee /tmp/eos-mnt/usr/local/bin/initialization.sh > /dev/null
	sudo chmod +x /tmp/eos-mnt/usr/local/bin/initialization.sh
else
	sudo cp ./build/initialization.sh /tmp/eos-mnt/usr/local/bin
fi

sudo cp ./build/initialization.service /tmp/eos-mnt/etc/systemd/system/initialization.service
sudo ln -s /etc/systemd/system/initialization.service /tmp/eos-mnt/etc/systemd/system/multi-user.target.wants/initialization.service

sudo umount /tmp/eos-mnt
