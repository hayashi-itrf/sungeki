"""Real loopback sockets: fast restart must not adopt another listener."""
import socket
import unittest
from supervisor import check_port_available


class PortAvailability(unittest.TestCase):
    def test_live_listener_is_rejected_and_untouched(self):
        for host in ('127.0.0.1', '0.0.0.0'):
            with self.subTest(host=host), socket.socket() as server:
                server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                server.bind((host, 0))
                server.listen(1)
                port = server.getsockname()[1]
                with self.assertRaises(OSError):
                    check_port_available(port)
                with socket.create_connection(('127.0.0.1', port), timeout=1):
                    client, _ = server.accept()
                    client.close()

    def test_server_active_close_does_not_prevent_immediate_restart(self):
        with socket.socket() as server:
            server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            server.bind(('127.0.0.1', 0))
            server.listen(1)
            port = server.getsockname()[1]
            with socket.create_connection(('127.0.0.1', port), timeout=1) as client:
                accepted, _ = server.accept()
                # Server closes first, leaving its endpoint in TIME_WAIT.
                accepted.close()
                self.assertEqual(client.recv(1), b'')
        check_port_available(port)
        with socket.socket() as restarted:
            restarted.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            restarted.bind(('127.0.0.1', port))
            restarted.listen(1)


if __name__ == '__main__':
    unittest.main()
