"""Sungeki-only loopback transport; the owning parent supplies a reserved socket.

Cancellation after dispatch requires the parent to stop/reap this owned process.
A disconnected client is not evidence that model inference has stopped.
"""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
import socket
import threading

MAX_REQUEST_BYTES = 128 * 1024


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON key')
        result[key] = value
    return result


def reject_constant(value):
    raise ValueError('nonfinite JSON number')


class OwnedServer(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = False

    def __init__(self, listen_fd, bridge):
        duplicate = os.dup(listen_fd)
        try:
            listener = socket.socket(fileno=duplicate)
        except Exception:
            os.close(duplicate)
            raise
        try:
            if (listener.family != socket.AF_INET
                    or listener.getsockopt(socket.SOL_SOCKET, socket.SO_TYPE) != socket.SOCK_STREAM
                    or listener.getsockname()[0] != '127.0.0.1'
                    or listener.getsockname()[1] == 0):
                raise ValueError('expected an inherited loopback listener')
            listener.listen(socket.SOMAXCONN)
            super().__init__(listener.getsockname(), Handler, bind_and_activate=False)
            self.socket.close()
            self.socket = listener
            self.bridge = bridge
            self.synthesis = threading.Lock()
            self.connections = threading.BoundedSemaphore(8)
        except Exception:
            listener.close()
            raise

    def process_request(self, request, address):
        if not self.connections.acquire(blocking=False):
            self.shutdown_request(request)
            return
        try:
            super().process_request(request, address)
        except Exception:
            self.connections.release()
            raise

    def process_request_thread(self, request, address):
        try:
            super().process_request_thread(request, address)
        finally:
            self.connections.release()


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def setup(self):
        super().setup()
        self.connection.settimeout(10)

    def log_message(self, *_args):
        pass  # Do not expose speech text or configured local paths in access logs.

    def reply(self, status, value, content_type='application/json'):
        body = value if isinstance(value, bytes) else json.dumps(value).encode('utf-8')
        self.close_connection = True
        try:
            self.send_response(status)
            self.send_header('Content-Type', content_type)
            self.send_header('Content-Length', str(len(body)))
            self.send_header('Connection', 'close')
            self.send_header('Cache-Control', 'no-store')
            self.end_headers()
            self.wfile.write(body)
        except (OSError, TimeoutError):
            pass  # Inference already settled; disconnect does not trigger another run.

    def local_request(self):
        hosts = self.headers.get_all('Host', [])
        expected = f'127.0.0.1:{self.server.server_address[1]}'
        if hosts != [expected] or self.headers.get('Origin') is not None:
            self.reply(403, {'error': 'local_client_required'})
            return False
        return True

    def do_GET(self):
        if not self.local_request():
            return
        if self.path == '/v1/health':
            self.reply(200, {'status': 'ready', 'busy': self.server.synthesis.locked()})
        elif self.path == '/v1/catalog':
            self.reply(200, self.server.bridge.catalog())
        else:
            self.reply(404, {'error': 'not_found'})

    def do_POST(self):
        if not self.local_request():
            return
        if self.path != '/v1/synthesize':
            self.reply(404, {'error': 'not_found'})
            return
        lengths = self.headers.get_all('Content-Length', [])
        if (self.headers.get('Transfer-Encoding') is not None or len(lengths) != 1
                or not lengths[0].isascii() or not lengths[0].isdigit()):
            self.reply(400, {'error': 'invalid_content_length'})
            return
        if len(lengths[0]) > 7 or not 0 < int(lengths[0]) <= MAX_REQUEST_BYTES:
            self.reply(413, {'error': 'request_too_large'})
            return
        if self.headers.get_all('Content-Type', []) != ['application/json']:
            self.reply(415, {'error': 'json_required'})
            return
        if not self.server.synthesis.acquire(blocking=False):
            self.reply(409, {'error': 'engine_busy'})
            return
        try:
            size = int(lengths[0])
            raw = self.rfile.read(size)
            if len(raw) != size:
                raise ValueError('incomplete request')
            payload = json.loads(raw.decode('utf-8'), object_pairs_hook=unique_object,
                                 parse_constant=reject_constant)
            # prepare validates before synthesize; errors never include raw paths/text.
            wav = self.server.bridge.synthesize_wav(payload)
            self.reply(200, wav, 'audio/wav')
        except (ValueError, UnicodeError, RecursionError):
            self.reply(422, {'error': 'invalid_synthesis_input_or_output'})
        except (OSError, TimeoutError):
            self.reply(408, {'error': 'request_io_failed'})
        except Exception:
            self.reply(500, {'error': 'synthesis_failed'})
        finally:
            self.server.synthesis.release()
