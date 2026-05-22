# based on the Python version by Isaac Gouy
#  that was based on a Java version:
#  Based on original version written in BCPL by Dr Martin Richards
#  in 1981 at Cambridge University Computer Laboratory, England
#  and a C++ version derived from a Smalltalk version written by
#  L Peter Deutsch.
#  Java version:  Copyright (C) 1995 Sun Microsystems, Inc.
#  Translation from C++, Mario Wolczko
#  Outer loop added by Alex Jacoby

# Copyright 2008-2010 Isaac Gouy
# Copyright (c) 2013, 2014, Regents of the University of California
# Copyright (c) 2018, 2021, Oracle and/or its affiliates.
# All rights reserved.
#
# Revised BSD license
#
# This is a specific instance of the Open Source Initiative (OSI) BSD license
# template http://www.opensource.org/licenses/bsd-license.php
#
# Redistribution and use in source and binary forms, with or without
# modification, are permitted provided that the following conditions are met:
#
#   Redistributions of source code must retain the above copyright notice, this
#   list of conditions and the following disclaimer.
#
#   Redistributions in binary form must reproduce the above copyright notice,
#   this list of conditions and the following disclaimer in the documentation
#   and/or other materials provided with the distribution.
#
#   Neither the name of "The Computer Language Benchmarks Game" nor the name of
#   "The Computer Language Shootout Benchmarks" nor the name "nanobench" nor the
#   name "bencher" nor the names of its contributors may be used to endorse or
#   promote products derived from this software without specific prior written
#   permission.
#
# THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
# AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
# IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
# DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE
# FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
# DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
# SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
# CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
# OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
# OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

# Task IDs
var I_IDLE = 1
var I_WORK = 2
var I_HANDLERA = 3
var I_HANDLERB = 4
var I_DEVA = 5
var I_DEVB = 6

# Packet types
var K_DEV = 1000
var K_WORK = 1001

# Packet

var BUFSIZE = 4

class Packet {
  Packet(l,i,k) {
    self.link = l
    self.ident = i
    self.kind = k
    self.datum = 0
    self.data = [0] * BUFSIZE
  }

  append_to(lst) {
    self.link = nil
    if lst == nil
        return self
    else {
      var p = lst
      var next = p.link
      while next != nil { 
        p = next
        next = p.link
      }
      p.link = self
      return lst
    }
  }
}

# Task Records

class TaskRec {
}

class DeviceTaskRec < TaskRec {
  DeviceTaskRec() {
    self.pending = nil
  }
}

class IdleTaskRec < TaskRec {
  IdleTaskRec() {
    self.control = 1
    self.count = 10000
  }
}

class HandlerTaskRec < TaskRec {
  HandlerTaskRec() {
    self.work_in = nil
    self.device_in = nil
  }

  workInAdd(p) {
    self.work_in = p.append_to(self.work_in)
    return self.work_in
  }

  deviceInAdd(p) {
    self.device_in = p.append_to(self.device_in)
    return self.device_in
  }
}

class WorkerTaskRec < TaskRec {
  WorkerTaskRec() {
    self.destination = I_HANDLERA
    self.count = 0
  }
}

# Task

class TaskState {
  TaskState() {
    self.packet_pending = true
    self.task_waiting = false
    self.task_holding = false
  }

  packetPending() {
    self.packet_pending = true
    self.task_waiting = false
    self.task_holding = false
    return self
  }

  waiting() {
    self.packet_pending = false
    self.task_waiting = true
    self.task_holding = false
    return self
  }

  running() {
    self.packet_pending = false
    self.task_waiting = false
    self.task_holding = false
    return self
  }

  waitingWithPacket() {
    self.packet_pending = true
    self.task_waiting = true
    self.task_holding = false
    return self
  }

  isPacketPending() {
    return self.packet_pending
  }

  isTaskWaiting() {
    return self.task_waiting
  }

  isTaskHolding() {
    return self.task_holding
  }

  isTaskHoldingOrWaiting() {
    return self.task_holding or (!self.packet_pending and self.task_waiting)
  }

  isWaitingWithPacket() {
    return self.packet_pending and self.task_waiting and !self.task_holding
  }
}

var tracing = false
var layout = 0

def trace(a) {
  layout -= 1
  if layout <= 0 {
    echo ''
    layout = 50
  }
  #echo '${a} '
}

var TASKTABSIZE = 10

class TaskWorkArea {
  TaskWorkArea() {
    self.taskTab = [nil] * TASKTABSIZE

    self.taskList = nil

    self.holdCount = 0
    self.qpktCount = 0
  }
}

var taskWorkArea = TaskWorkArea()

class Task < TaskState {

  Task(i,p,w,initialState,r) {
    self.link = taskWorkArea.taskList
    self.ident = i
    self.priority = p
    self.input = w

    self.packet_pending = initialState.isPacketPending()
    self.task_waiting = initialState.isTaskWaiting()
    self.task_holding = initialState.isTaskHolding()

    self.handle = r

    taskWorkArea.taskList = self
    taskWorkArea.taskTab[i] = self
  }

  fn(pkt,r) {
    raise Exception('not implemented')
  }

  addPacket(p,old) {
    if self.input == nil {
      self.input = p
      self.packet_pending = true
      if self.priority > old.priority return self
    } else p.append_to(self.input)
    return old
  }


  runTask() {
    var msg
    if self.isWaitingWithPacket() {
      msg = self.input
      self.input = msg.link
      if self.input == nil
        self.running()
      else
        self.packetPending()
    } else msg = nil

    return self.fn(msg,self.handle)
  }


  waitTask() {
    self.task_waiting = true
    return self
  }


  hold() {
    taskWorkArea.holdCount += 1
    self.task_holding = true
    return self.link
  }


  release(i) {
    var t = self.findtcb(i)
    t.task_holding = false
    if t.priority > self.priority
      return t
    else
      return self
  }


  qpkt(pkt) {
    var t = self.findtcb(pkt.ident)
    taskWorkArea.qpktCount += 1
    pkt.link = nil
    pkt.ident = self.ident
    return t.addPacket(pkt,self)
  }


  findtcb(id) {
    var t = taskWorkArea.taskTab[id]
    if t == nil
      raise Exception("Bad task id ${id}")
    return t
  }
}

# DeviceTask


class DeviceTask < Task {
  DeviceTask(i,p,w,s,r) {
    parent(i,p,w,s,r)
  }

  fn(pkt,r) {
    var d = r
    assert instance_of(d, DeviceTaskRec)
    if pkt == nil {
      pkt = d.pending
      if pkt == nil return self.waitTask()
      else {
        d.pending = nil
        return self.qpkt(pkt)
      }
    } else {
      d.pending = pkt
      if tracing trace(pkt.datum)
      return self.hold()
    }
  }
}

class HandlerTask < Task {
  HandlerTask(i,p,w,s,r) {
    parent(i,p,w,s,r)
  }

  fn(pkt,r) {
    var h = r
    assert instance_of(h, HandlerTaskRec)
    if pkt != nil {
      if pkt.kind == K_WORK h.workInAdd(pkt)
      else h.deviceInAdd(pkt)
    }
    var work = h.work_in
    if work == nil return self.waitTask()
    var count = work.datum
    if count >= BUFSIZE {
      h.work_in = work.link
      return self.qpkt(work)
    }

    var dev = h.device_in
    if dev == nil return self.waitTask()

    h.device_in = dev.link
    dev.datum = work.data[count]
    work.datum = count + 1
    return self.qpkt(dev)
  }
}

# IdleTask


class IdleTask < Task {
  IdleTask(i,p,w,s,r) {
    parent(i,0,nil,s,r)
  }

  fn(pkt,r) {
    var i = r
    assert instance_of(i, IdleTaskRec)
    i.count -= 1
    if i.count == 0 return self.hold()
    else if i.control & 1 == 0 {
      i.control //= 2
      return self.release(I_DEVA)
    } else {
      i.control = i.control // 2 ^ 0xd008
      return self.release(I_DEVB)
    }
  }
}

# WorkTask

var A = ord('A')

class WorkTask < Task {
  WorkTask(i,p,w,s,r) {
    parent(i,p,w,s,r)
  }

  fn(pkt,r) {
    var w = r
    assert instance_of(w, WorkerTaskRec)
    if pkt == nil return self.waitTask()

    var dest
    if w.destination == I_HANDLERA
      dest = I_HANDLERB
    else
      dest = I_HANDLERA

    w.destination = dest
    pkt.ident = dest
    pkt.datum = 0

    for i in 0..BUFSIZE {
      w.count += 1
      if w.count > 26 w.count = 1
      pkt.data[i] = A + w.count - 1
    }

    return self.qpkt(pkt)
  }
}

def schedule() {
  var t = taskWorkArea.taskList
  while t != nil {
    var pkt = nil

    if tracing {
      echo "tcb = ${t.ident}"
    }

    if t.isTaskHoldingOrWaiting() {
      t = t.link
    } else {
      if tracing trace(chr(ord("0") + t.ident))
      t = t.runTask()
    }
  }
}

def run() {
  taskWorkArea.holdCount = 0
  taskWorkArea.qpktCount = 0

  IdleTask(I_IDLE, 1, 10000, TaskState().running(), IdleTaskRec())

  var wkq = Packet(nil, 0, K_WORK)
  wkq = Packet(wkq , 0, K_WORK)
  WorkTask(I_WORK, 1000, wkq, TaskState().waitingWithPacket(), WorkerTaskRec())

  wkq = Packet(nil, I_DEVA, K_DEV)
  wkq = Packet(wkq , I_DEVA, K_DEV)
  wkq = Packet(wkq , I_DEVA, K_DEV)
  HandlerTask(I_HANDLERA, 2000, wkq, TaskState().waitingWithPacket(), HandlerTaskRec())

  wkq = Packet(nil, I_DEVB, K_DEV)
  wkq = Packet(wkq , I_DEVB, K_DEV)
  wkq = Packet(wkq , I_DEVB, K_DEV)
  HandlerTask(I_HANDLERB, 3000, wkq, TaskState().waitingWithPacket(), HandlerTaskRec())

  wkq = nil
  DeviceTask(I_DEVA, 4000, wkq, TaskState().waiting(), DeviceTaskRec())
  DeviceTask(I_DEVB, 5000, wkq, TaskState().waiting(), DeviceTaskRec())

  schedule()

  return taskWorkArea.holdCount == 9297 and taskWorkArea.qpktCount == 23246
}


####### TESTING .......
var start = time()
run()
echo 'Time taken ${time() - start}s'
