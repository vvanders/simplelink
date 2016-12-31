module SimpleLink exposing (..)

retry_timeout : Int
retry_timeout = 500

retry_count : Int
retry_count = 4

type Msg = Retry RetryMsg
  | Expire PRN
  | Ack AckMsg
  | Recv RecvMsg
  | Observe RecvMsg
  | Send SendMsg

type alias PRN = Int

type alias RetryMsg = {
  prn: Int,
  next_retry: Int
}

type alias RecvMsg = {
  route : List String,
  prn : Int,
  msg : String
}

type alias SendMsg = RecvMsg

type alias AckMsg = {
  route : List String,
  prn: Int
}