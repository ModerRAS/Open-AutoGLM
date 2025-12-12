# Phone Agent Docker Image
# Multi-stage build for minimal image size

FROM alpine:3.19 AS runtime

# Install required runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    android-tools \
    && rm -rf /var/cache/apk/*

# Create non-root user
RUN adduser -D -u 1000 agent
USER agent
WORKDIR /home/agent

# Copy pre-built binary (passed as build arg)
ARG BINARY_PATH=phone-agent
COPY --chown=agent:agent ${BINARY_PATH} /usr/local/bin/phone-agent
RUN chmod +x /usr/local/bin/phone-agent

# Copy resources (font file for calibration)
COPY --chown=agent:agent resources/ /home/agent/resources/

# Default environment variables
ENV MODEL_BASE_URL=http://localhost:8000/v1 \
    MODEL_API_KEY=EMPTY \
    MODEL_NAME=autoglm-phone-9b \
    AGENT_LANG=cn \
    COORDINATE_SCALE=1.61

# Expose ADB default port (for adb over network)
EXPOSE 5037

ENTRYPOINT ["phone-agent"]
CMD ["--help"]
