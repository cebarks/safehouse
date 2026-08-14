FROM registry.fedoraproject.org/fedora-minimal:44

# Runtime deps for PZ native libs (64-bit) + SteamCMD (32-bit)
RUN microdnf install -y \
        libstdc++ libX11 libxcb libXext libSM libICE \
        glibc.i686 libstdc++.i686 \
        tar gzip curl \
    && microdnf clean all

# Install SteamCMD
RUN mkdir -p /steamcmd \
    && curl -fsSL https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz \
       | tar xz -C /steamcmd

# PZ server will be installed into /server via volume or safehouse setup
# World data lives in /zomboid
VOLUME ["/server", "/zomboid"]

# Replace start-server.sh — these two env vars are all it does
ENV LD_LIBRARY_PATH=/server/linux64:/server:/server/jre64/lib:/server/jre64/lib/server
ENV PATH=/server/jre64/bin:/steamcmd:$PATH

WORKDIR /server
EXPOSE 16261/udp 16262/udp 27015/tcp

# PID 1 = Java process directly. No shell wrapper, no signal mismatch.
ENTRYPOINT ["./ProjectZomboid64"]
