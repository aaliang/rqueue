# rqueue
Experimental central messaging server designed for high throughput.

### TCP protocol
Each message is prefixed by five bytes, called the `preamble`.
- The first four bytes bytes `message[0:3]`, concatenated together forms a 32-bit big-endian unsigned integer that represents the number of bytes `payload_length` that follow the `preamble`.
- The next byte `message[4]` signifies the type of the message `message_type`. Some relevant values are listed below.
- The next `payload_length` bytes after the `preamble` comprise the `payload`

```
PUBLISH       = 0    # publishes a message on a topic
SUBSCRIBE     = 1    # subscribes a client to a topic
REMOVE        = 2    # removes client intent on topic
DEREGISTER    = 5    # purges all subscriptions for a client
NOTIFICATION  = 7    # message pertaining to topic sent from server to client
```

####`NOTIFICATION`

`Server |> Client`
When a publish message is received by the server, a notification is sent out to each client that has previously sent a `SUBSCRIBE`.

|`NOTIFICATION`| payload_len | message_type| topic_len | topic | content
|---           |---          |---          | ---       | ---   | --- 
**`LENGTH`**   |  4          | 1           | 1         |  T    |  C
**`VAL`**      | T + C + 1   | 7           |           |       |


####`SUBSCRIBE`

`Client |> SERVER`
Registers interest in a topic

|`SUBSCRIBE`   | payload_len | message_type| topic_len | topic
|---           |---          |---          | ---       | ---
**`LENGTH`**   |  4          | 1           | 1         |  T
**`VAL`**      | T + 1       | 1           |           |


####`PUBLISH`

`Client |> Server`
Clients send a publish message when they have something to announce.
The payload of a publish message is a valid `NOTIFICATION` message. That is to say, `PUBLISH` is composed with `NOTIICATION`

|`PUBLISH`     | payload_len | message_type | notify_len | notify_type | topic_len | topic | content
|---           |---          |---           | ---        | ---         | ---       | ---   | ---
**`LENGTH`**   | 4           | 1            | 4          | 1           | 1         | T     | C
**`VAL`**      | T+C+6       | 0            |            | 7           |           |       |


can be thought of as

|`PUBLISH`     | payload_length | message_type = 0 | `NOTIFICATION`
|---           |---             |---               | ---
**`LENGTH`**   |  4             | 1                | N
**`VAL`**      | N              | 0                |

####`REMOVE`
`Client |> Server`
Removes a subcription for this client

|`REMOVE`      | payload_length | message_type     | topic_len | topic
|---           |---             |---               | ---       | ---
**`LENGTH`**   |  4             | 1                | 1         |  T
**`VAL`**      | T + 1          | 2                |           |

####`DEREGISTER`
`Client |> Server`
Removes all subscriptions for this client.

|`DEREGISTER`| payload_length | message_type = 2
|---         |---             |---
**`LENGTH`** |  4             | 1
**`VAL`**    |  0000          |

When the client is disconnected, subscriptions are automatically disconnected

Payloads are capped at 2KB, though you are encouraged to stay under to stay under ethernet's MTU of 1500 bytes for safety. (TCP reads and writes on the client are potentially unstable). As a result, ```payload_length``` will likely decrease to a 16-bit unsigned integer

Larger payloads will be supported in form of multi-part messages.


### Usage
#### executable:
```.sh
  ./server # default configuration listens on port 6567
  ./server --port 5000 --threads 8
```

#### cargo:
```.sh
  cargo run --bin server
  cargo run --release --bin server
  cargo run --release --bin server -- --port 5000 --threads 8
```



#### benchmarking
```.sh
  cargo run --bin server &  # then
  cargo run --bin sink &    # then
  cargo run --bin pusher
```
  
