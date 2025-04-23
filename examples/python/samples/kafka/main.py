from adder import exports
from adder.imports import say, ttbox as box, msgpack, tuple as tt_tuple
from adder.imports.tarantool_tarantool_types import *
from inspect import currentframe

class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        print("HTTP HANDLE")

import json
import sys
import my_zlib_component as zlib
sys.modules["zlib"] = zlib

# Encode an empty string with 'idna' as WASM needs this encoding
"".encode('idna')

def encode(obj):
    try:
        return msgpack.encode(json.dumps(obj).encode('utf-8'))
    except Exception as error:
        say.say_error(f"PY | Error in encode: {json.dumps({'error': str(error)}, indent=2)}")
        raise


def decode(*, data = None, ptr = None):
    try:
        if ptr:
            decoded_json_bytes: bytes = msgpack.decode_from_raw_ptr(ptr)
        elif data:
            decoded_json_bytes: bytes = msgpack.decode(data)
        decoded_json_string = decoded_json_bytes.decode('utf-8')
        return json.loads(decoded_json_string)
    except Exception as error:
        say.say_error(f"PY | Error in decode: {json.dumps({'error': str(error)}, indent=2)}")
        raise

import socket

def check_kafka_connection(host='172.17.0.1', port=9092, timeout=5):
    try:
        sa = (host, port)
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM, 0)
        sock.connect(sa)
        sock.close()
        return True
    except (socket.timeout, socket.error) as e:
        return False

import socket

saved_getaddrinfo = socket.getaddrinfo

def _getaddrinfo(host, port, *args, **kwargs):
    res = saved_getaddrinfo(host, port, *args, **kwargs)
    updated_res = []
    for item in res:
        family, socktype, proto, canonname, sockaddr = item

        if isinstance(sockaddr, tuple) and len(sockaddr) == 2:  # IPv4: ('127.0.0.1', 9092)
            updated_sockaddr = (sockaddr[0], port)
        elif isinstance(sockaddr, tuple) and len(sockaddr) == 4:  # IPv6: ('::1', 9092, 0, 0)
            updated_sockaddr = (sockaddr[0], port, sockaddr[2], sockaddr[3])
        elif isinstance(sockaddr, tuple) and len(sockaddr) == 1:  # Other formats: (IPv4 address)
            updated_sockaddr = (sockaddr[0], port)
        else:
            updated_sockaddr = sockaddr  # If it's already in expected format

        updated_res.append((family, socktype, proto, canonname, updated_sockaddr))

    return updated_res

socket.getaddrinfo = _getaddrinfo

import kafka

def kafka_test():
    say.say_info(f"Check Kafka connection: {check_kafka_connection()}", None)
    consumer = kafka.KafkaConsumer(
        'tarantool',
        bootstrap_servers=['172.17.0.1:9092'],
        group_id='my-consumer-group',
        value_deserializer=lambda x: json.loads(x.decode('utf-8'))
    )
    consumed = 0
    for message in consumer:
        value = message.value
        say.say_info(f"{consumed=} | GET MESSAGE FROM KAFKA {value}", None)
        consumed += 1

# from confluent_kafka import Consumer, KafkaException, KafkaError

# conf = {
#     'bootstrap.servers': 'localhost:9092',
#     'group.id': 'my_group',
#     'auto.offset.reset': 'earliest',
#     'enable.auto.commit': 'false',  # отключить автоматическое подтверждение сообщений
#     'session.timeout.ms': 6000,  # уменьшить время ожидания heartbeat, чтобы не использовать поток
#     'heartbeat.interval.ms': 1000,  # интервал heartbeat
#     'max.poll.interval.ms': 5000,  # интервал между вызовами poll
#     'queued.max.messages.kbytes': 1000,  # увеличить максимальное количество сообщений
#     'queued.max.messages': 1000,  # увеличить максимальное количество сообщений
#     'consume.on.fatal.error': True  # обработка ошибок без использования потоков
# }

# def confluent_kafka_test():
#     consumer = Consumer(conf)
#     consumer.subscribe(['tarantool'])

#     try:
#         while True:
#             msg = consumer.poll(timeout=1.0)
#             if msg is None:
#                 continue
#             if msg.error():
#                 if msg.error().code() == KafkaError._PARTITION_EOF:
#                     print(f"End of partition reached {msg.partition}")
#                 else:
#                     raise KafkaException(msg.error())
#             else:
#                 print(f"Received message: {msg.value().decode('utf-8')}")
#     finally:
#         consumer.close()


class Run(exports.Run):
    def run(self) -> None:
        try:
            kafka_test()
        except Exception as e:
            say.say_error(str(e.with_traceback()), None)
