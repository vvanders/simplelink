module SimpleLink exposing (..)

type Msg = Retry PRN
  | Expire PRN
  | Ack AckMsg
  | Recv RecvMsg
  | Observe RecvMsg

type alias PRN = Int

type alias SendMsg = {
  route : List String,
  msg : String
}

type alias RecvMsg = {
  route : List String,
  prn : Int,
  msg : String
}

type alias AckMsg = {
  route : List String,
  prn: Int
}