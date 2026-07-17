sequenceDiagram
    participant "ComplexController" as p0
    participant "ComplexTicketService" as p1

    activate p0
    p0->>p1: crudTicket()
    activate p1
    p1->>p1: getMapTicketToXML()
    activate p1
    p1-->>p1: Map<String, Object>
    deactivate p1
    p1->>p1: parseTickets()
    activate p1
    p1-->>p1: List<Ticket>
    deactivate p1
    p1->>p1: insertTicket()
    activate p1
    p1-->>p1: boolean
    deactivate p1
    p1->>p1: modificarTicket()
    activate p1
    p1-->>p1: boolean
    deactivate p1
    p1->>p1: logError()
    activate p1
    p1-->>p1: logError
    deactivate p1
    p1-->>p0: boolean
    deactivate p1
    deactivate p0
