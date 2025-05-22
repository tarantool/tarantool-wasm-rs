if __name__ == "__main__":
    from kafka import KafkaConsumer
    import json

    # Настройка Kafka Consumer
    consumer = KafkaConsumer(
        'tarantool',  # Топик, из которого будем читать
        bootstrap_servers=['localhost:9092'],  # Убедитесь, что это правильный адрес
        group_id='my-consumer-group',
        value_deserializer=lambda x: json.loads(x.decode('utf-8'))
    )

    # Печать полученных сообщений
    for message in consumer:
        print(f"Received message: {message.value}")
