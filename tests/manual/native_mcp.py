"""Attach native GUI checks to an existing host without starting a replacement.

Wait for the GUI's normal project/file connection before using this client.
The integration-test McpClient starts its own isolated data directory by default;
that behavior must not be used to bootstrap a native app's shared endpoint.
"""
from pathlib import Path
from support import McpClient, connect, exchange, hello


class NativeMcpClient(McpClient):
    def __init__(self, directory: Path, *, response_timeout=10):
        # Fail before launching MCP if the native host has not bound its socket.
        with connect(directory) as connection:
            response = exchange(connection, hello())
        observed = response.get('host')
        if not observed or observed.get('application') != 'sungeki-host':
            raise RuntimeError('Native host identity is unavailable')
        # MCP can reconnect to the observed socket, but cannot auto-start a host
        # with the isolated data settings inherited from integration support.
        disabled_binary = directory / '.native-attach-only-no-host'
        if disabled_binary.exists() or disabled_binary.is_symlink():
            raise RuntimeError('Attach-only sentinel path must not exist')
        super().__init__(directory, binary=disabled_binary, response_timeout=response_timeout)
        try:
            result = self.capabilities()
            actual = result.get('structuredContent', {})
            if result.get('isError') or actual.get('host_instance_id') != observed['host_instance_id']:
                raise RuntimeError('Native host changed while attaching; reconnect through the GUI first')
        except BaseException:
            self.close()
            raise
        self.observed_host = observed
