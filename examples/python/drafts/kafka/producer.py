from kafka import KafkaProducer
import json

producer = KafkaProducer(
    bootstrap_servers=['localhost:9092'],
    value_serializer=lambda v: json.dumps(v).encode('utf-8')
)

test_data = [
    [1, "Alice", 25],
    [2, "Bob", 30],
    [3, "Charlie", 35],
    [4, "David", 40],
    [5, "Eve", 4],
]

for row in test_data:
    producer.send('tarantool', value=row)

producer.flush()
producer.close()

print("Message sent successfully.")
