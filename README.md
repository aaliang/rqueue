# rqueue
Experimental central messaging server designed for high throughput.

### TCP protocol
- Each message is prefixed by three bytes, called the `preamble`.
- The first two bytes bytes `message[0:2]`, concatenated together forms a 16-bit big-endian unsigned integer that represents the number of bytes `payload_length` that follow the `preamble`.
- The next byte `message[2]` signifies the type of the message `message_type`. Some relevant values are listed below.
- The next `payload_length` bytes after the `preamble` comprise the `payload`

```
SUBSCRIBE     = 1    # subscribes a client to a topic
REMOVE        = 2    # removes client intent on topic
DEREGISTER    = 5    # purges all subscriptions for a client
NOTIFICATION  = 7    # message pertaining to topic
```

####`NOTIFICATION` & `PUBLISH`

`Client |> Server`
Clients send a notification message when they have something to announce.

`Server |> Client`
When a notification message is received by the server, the same notification is sent to each client that has previously sent a `SUBSCRIBE`.


|`NOTIFICATION`| payload_length | message_type| topic_len | topic | content
|---           |---          |---          | ---       | ---   | --- 
**`LENGTH`**   |  4          | 1           | 1         |  T    |  C
**`VAL`**      | T + C + 1   | 7           |           |       |

`topic_len` is the length, in bytes of the topic. The topic is capped at 8-bits. Everything after the topic (up to the `payload_len` offset) is assumed to be the content.


####`SUBSCRIBE`

`Client |> SERVER`
Registers interest in a topic

|`SUBSCRIBE`   | payload_length | message_type| topic_len | topic
|---           |---          |---          | ---       | ---
**`LENGTH`**   |  4          | 1           | 1         |  T
**`VAL`**      | T + 1       | 1           |           |

TODO: the topic_len is perhaps not useful, as it can be derived from the payload_len in this case

####`REMOVE`
`Client |> Server`
Removes a subcription for this client

|`REMOVE`      | payload_length | message_type  | topic
|---           |---             |---            | ---
**`LENGTH`**   |  4             | 1             |  T
**`VAL`**      | T              | 2             |
unimplemented for now

####`DEREGISTER`
`Client |> Server`
Removes all subscriptions for this client.

|`DEREGISTER`| payload_length | message_type
|---         |---             |---
**`LENGTH`** |  4             | 1
**`VAL`**    |  0000          | 5

When a client is disconnected its subscriptions are automatically purged.

Payloads are capped at 2KB, though you are encouraged to stay under to stay under ethernet's MTU of 1500 bytes for safety. (TCP reads and writes on the client are potentially unstable). Larger payloads will be supported in form of multi-part messages.


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
note: this is not a very good benchmark as it places a writer, consumer, and the server on the same box. Nor is the writer especially high throughput

#### misc
certain guarantees will eventually be turned off/on that will radically effect server throughput
