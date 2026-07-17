sequenceDiagram
    participant "TestController" as p0
    participant "TicketCreationService" as p1

    activate p0
    p0->>p1: crudTicket()
    activate p1
    p1->>p1: getMapTicketToXML()
    activate p1
    p1-->>p1: Map<String, Object>
    deactivate p1
    p1->>p1: insertTicket()
    activate p1
    p1-->>p1: insertTicket
    deactivate p1
    p1->>p1: modificarTicket()
    activate p1
    p1-->>p1: modificarTicket
    deactivate p1
    p1-->>p0: boolean
    deactivate p1
    deactivate p0
