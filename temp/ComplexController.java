package com.example.api;

import org.springframework.web.bind.annotation.*;
import com.example.service.ComplexTicketService;

@RestController
@RequestMapping("/api/v2")
public class ComplexController {
    
    private ComplexTicketService service;
    
    @PostMapping("/crudTicket")
    public String handle(@RequestBody String xml) {
        boolean result = service.crudTicket(xml);
        return result ? "ok" : "fail";
    }
}
