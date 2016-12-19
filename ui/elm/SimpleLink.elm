module SimpleLink exposing (..)

type Msg = Retry PRN
  | Expire PRN
  | Ack AckMsg
  | Recv RecvMsg
  | Observe RecvMsg
  | Send SendMsg

type alias PRN = Int

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